{
  description = "PluresDB - P2P Graph Database with SQLite Compatibility";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        pluresdb = pkgs.callPackage ./default.nix { };
      in
      {
        packages = {
          default = pluresdb;
          pluresdb = pluresdb;
        };

        apps = {
          default = flake-utils.lib.mkApp {
            drv = pluresdb;
            exePath = "/bin/pluresdb";
          };
          serve = flake-utils.lib.mkApp {
            drv = pluresdb;
            exePath = "/bin/pluresdb-server";
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            deno
            nodejs
            npm
            git
          ];
        };

        formatter = pkgs.nixpkgs-fmt;
      });
}
