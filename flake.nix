{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };

        nost = naersk-lib.buildPackage {
          src = ./.;

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
          buildInputs = with pkgs; [
            openssl
            openssl.dev
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
        };

        dockerImage = pkgs.dockerTools.buildLayeredImage {
          name = "nost";
          tag = "latest";
          contents = [ nost ];
          config = {
            Cmd = [ "${nost}/bin/nost" ];
          };
        };
      in
      with pkgs;
      {
        packages = {
          inherit nost dockerImage;
          default = nost;
        };
        devShell = mkShell {
          buildInputs = with pkgs; [
            nost cargo rustc rustfmt rustPackages.clippy dive just
            pkg-config openssl openssl.dev curl libclang
          ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
        };
      }
    );
}

