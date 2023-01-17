{
  description = "antikoerper flake";
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-22.05";
    flake-utils = {
      url = "github:numtide/flake-utils";
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, nixpkgs, crane, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustTarget = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustTarget;

        tomlInfo = craneLib.crateNameFromCargoToml { cargoToml = ./Cargo.toml; };
        inherit (tomlInfo) pname version;
        src = ./.;

        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src;
          inherit nativeBuildInputs;
        };

        binary = craneLib.buildPackage {
          inherit cargoArtifacts src pname version;
          inherit nativeBuildInputs;
          doCheck = false;
        };

        nativeBuildInputs = with pkgs; [
          openssl
          pkgconfig
        ];

      in
      rec {
        checks = {
          inherit binary;

          binary-clippy = craneLib.cargoClippy {
            inherit cargoArtifacts src;
            inherit nativeBuildInputs;
            cargoClippyExtraArgs = "-- --deny warnings";
          };

          binary-fmt = craneLib.cargoFmt {
            inherit src;
          };
        };

        apps.binary = flake-utils.lib.mkApp {
          name = pname;
          drv = binary;
        };

        apps.default = apps.binary;

        packages.binary = binary;
        packages.default = packages.binary;

        devShells.default = devShells.binary;
        devShells.binary = pkgs.mkShell {
          nativeBuildInputs = nativeBuildInputs ++ [
            rustTarget
          ];
        };
      }
    );
}
