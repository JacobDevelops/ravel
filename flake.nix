{
  description = "Ravel reproducible development environment";

  inputs = {
    nixpkgs.url = "github:cachix/devenv-nixpkgs/rolling";

    devenv = {
      url = "github:cachix/devenv/v2.1.2";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  nixConfig = {
    extra-substituters = "https://devenv.cachix.org";
    extra-trusted-public-keys = "devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw=";
  };

  outputs =
    inputs@{
      self,
      devenv,
      nixpkgs,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-darwin"
      ];
      forEachSystem = nixpkgs.lib.genAttrs systems;
      mkShell =
        system:
        devenv.lib.mkShell {
          inherit inputs;
          pkgs = nixpkgs.legacyPackages.${system};
          modules = [
            ./devenv.nix
            {
              devenv.root =
                let
                  workingDirectory = builtins.getEnv "PWD";
                in
                if workingDirectory == "" then builtins.toString self else workingDirectory;
            }
          ];
        };
    in
    {
      devShells = forEachSystem (system: {
        default = mkShell system;
      });

      checks = forEachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          shell = mkShell system;
        in
        {
          devenv = shell.ci;
          scaffold = pkgs.stdenv.mkDerivation {
            pname = "ravel-scaffold-check";
            version = "0";
            src = self;
            cargoDeps = pkgs.rustPlatform.importCargoLock {
              lockFile = ./Cargo.lock;
            };
            nativeBuildInputs = [
              pkgs.jq
              pkgs.rustPlatform.cargoSetupHook
              shell.config.languages.rust.toolchainPackage
            ];
            buildPhase = ''
              runHook preBuild
              export HOME="$TMPDIR"
              cargo run --offline --locked -p ravel-xtask -- verify scaffold
              runHook postBuild
            '';
            installPhase = ''
              runHook preInstall
              mkdir -p "$out"
              touch "$out/passed"
              runHook postInstall
            '';
          };
        }
      );
    };
}
