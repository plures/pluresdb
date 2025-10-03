{
  description = "Rusty Gun - P2P Graph Database with SQLite Compatibility";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        rusty-gun = pkgs.callPackage ./default.nix { };
      in
      {
        packages = {
          default = rusty-gun;
          rusty-gun = rusty-gun;
        };

        apps = {
          default = flake-utils.lib.mkApp {
            drv = rusty-gun;
            exePath = "/bin/rusty-gun";
          };
          serve = flake-utils.lib.mkApp {
            drv = rusty-gun;
            exePath = "/bin/rusty-gun-server";
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
