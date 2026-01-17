{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
    geni.url = "github:emilpriver/geni";
  };

  outputs = {
    self,
    nixpkgs,
    utils,
    naersk,
    geni,
  }:
    utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {inherit system;};
        naersk-lib = pkgs.callPackage naersk {};

        nost = naersk-lib.buildPackage {
          src = ./.;

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
          buildInputs = with pkgs; [
            openssl
            openssl.dev
            sqlite
          ];

          override = x: {
            cargoVendorDir = builtins.toFile "vendor-config" ''
              [patch.crates-io]
              twitch_types = { git = "https://github.com/twitch-rs/twitch_api" }
              twitch_oauth2 = { git = "https://github.com/twitch-rs/twitch_api" }
            '';
          };

          copyLibs = true;
          doDoc = false;
          gitSubmodules = true;
          gitAllRefs = true;

          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          LIBSQL_FORCE_SYSTEM_SQLITE = "1";
        };

        dockerImage = pkgs.dockerTools.buildLayeredImage {
          name = "nost";
          tag = "latest";
          contents = [nost];
          config = {
            Cmd = ["${nost}/bin/nost"];
          };
        };
      in
        with pkgs; {
          packages = {
            inherit nost dockerImage;
            default = nost;
          };
          devShell = mkShell {
            buildInputs = with pkgs; [
              cargo
              rustc
              rustfmt
              rustPackages.clippy
              dive
              just
              pkg-config
              openssl
              openssl.dev
              curl
              libclang
              sqlite
              geni.packages.${system}.geni
            ];
            RUST_SRC_PATH = rustPlatform.rustLibSrc;
            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
            LIBSQL_FORCE_SYSTEM_SQLITE = "1";
          };
        }
    );
}
