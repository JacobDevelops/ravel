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
          scaffold =
            pkgs.runCommand "ravel-scaffold-check"
              {
                nativeBuildInputs = [
                  pkgs.jq
                  shell.config.languages.rust.toolchainPackage
                ];
                src = self;
              }
              ''
                export HOME="$TMPDIR"
                cp -R "$src" source
                chmod -R u+w source
                cd source
                cargo run --offline --locked -p ravel-xtask -- verify scaffold
                touch "$out"
              '';
        }
      );
    };
}
