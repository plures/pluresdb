{
  description = "PluresDB development and build flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        workspaceCargo = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        pluresVersion = workspaceCargo.workspace.package.version;

        ortDist = {
          x86_64-linux = {
            url = "https://cdn.pyke.io/0/pyke:ort-rs/ms@1.23.2/x86_64-unknown-linux-gnu.tar.lzma2";
            hash = "sha256-jFfQWaqu5AeBKladbXDGefCQrWnhoUIEMJ6ALcu6o18=";
          };
          aarch64-linux = {
            url = "https://cdn.pyke.io/0/pyke:ort-rs/ms@1.23.2/aarch64-unknown-linux-gnu.tar.lzma2";
            hash = "sha256-wlJIwy2E8ii51YS4SzHhV35IENRr618wTp+jQMAAF2w=";
          };
        }.${system};

        onnxruntimeArchive = pkgs.fetchurl {
          inherit (ortDist) url hash;
        };

        onnxruntimeLib = pkgs.stdenvNoCC.mkDerivation {
          pname = "onnxruntime-libonnxruntime-a";
          version = "1.23.2";
          src = onnxruntimeArchive;

          nativeBuildInputs = [ pkgs.python3 ];
          dontUnpack = true;

          installPhase = ''
            runHook preInstall

            mkdir -p "$out/lib"

            python - <<'PY'
            import io
            import lzma
            import os
            import pathlib
            import tarfile

            src = pathlib.Path(os.environ["src"])
            out = pathlib.Path(os.environ["out"]) / "lib" / "libonnxruntime.a"

            compressed = src.read_bytes()
            tar_bytes = lzma.decompress(
                compressed,
                format=lzma.FORMAT_RAW,
                filters=[{"id": lzma.FILTER_LZMA2, "dict_size": 1 << 26}],
            )

            with tarfile.open(fileobj=io.BytesIO(tar_bytes), mode="r:") as tf:
                member = next(
                    (m for m in tf.getmembers() if m.isfile() and m.name.endswith("libonnxruntime.a")),
                    None,
                )
                if member is None:
                    raise RuntimeError("libonnxruntime.a not found in ONNX Runtime archive")

                extracted = tf.extractfile(member)
                if extracted is None:
                    raise RuntimeError("failed to extract libonnxruntime.a")

                out.write_bytes(extracted.read())
            PY

            runHook postInstall
          '';
        };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "pluresdb-cli";
          version = pluresVersion;

          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          cargoBuildFlags = [ "-p" "pluresdb-cli" ];

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.openssl ];

          ORT_LIB_LOCATION = "${onnxruntimeLib}/lib";
        };

        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.clippy
            pkgs.rustfmt
            pkgs.pkg-config
            pkgs.openssl
          ];

          ORT_LIB_LOCATION = "${onnxruntimeLib}/lib";
        };
      });
}
