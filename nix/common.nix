{
  config,
  lib,
  pkgs,
  ...
}:

let
  supportedSystems = [
    "x86_64-linux"
    "aarch64-darwin"
  ];
  milestones = map (number: "M${toString number}") (lib.range 0 13);
in
{
  assertions = [
    {
      assertion = builtins.elem pkgs.stdenv.hostPlatform.system supportedSystems;
      message = "Ravel supports only x86_64-linux and aarch64-darwin during M0.";
    }
  ];

  env = {
    CARGO_TERM_COLOR = "always";
    RUST_BACKTRACE = "1";
  };

  languages.rust = {
    enable = true;
    toolchainFile = ../rust-toolchain.toml;
  };

  packages = with pkgs; [
    cargo-deny
    cargo-llvm-cov
    cargo-nextest
    check-jsonschema
    coreutils
    curl
    git
    gnutar
    gzip
    hyperfine
    jq
    jujutsu
    markdownlint-cli
    nixfmt
    pkg-config
    shellcheck
    shfmt
    statix
    taplo
    unzip
    xz
    zip
    zstd
  ];

  scripts = {
    ravel-format = {
      description = "Check Rust, Nix, shell, TOML, and Markdown formatting.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        cargo fmt --all --check
        nixfmt --check devenv.nix flake.nix nix/*.nix
        shfmt -d .envrc

        mapfile -d "" toml_files < <(
          find . -type f -name "*.toml" \
            -not -path "./.devenv/*" \
            -not -path "./.direnv/*" \
            -not -path "./.jj/*" \
            -not -path "./target/*" \
            -print0
        )
        taplo format --check "''${toml_files[@]}"

        mapfile -d "" markdown_files < <(
          find . -type f -name "*.md" \
            -not -path "./.devenv/*" \
            -not -path "./.direnv/*" \
            -not -path "./.jj/*" \
            -not -path "./target/*" \
            -print0
        )
        markdownlint "''${markdown_files[@]}"
      '';
    };

    ravel-typecheck = {
      description = "Typecheck every workspace target and feature.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        cargo check --workspace --all-targets --all-features
      '';
    };

    ravel-lint = {
      description = "Run Rust, Nix, and shell static analysis.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        cargo clippy --workspace --all-targets --all-features -- -D warnings
        statix check .
        shellcheck .envrc
      '';
    };

    ravel-test = {
      description = "Run workspace and documentation tests.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        cargo nextest run --workspace --all-features
        cargo test --workspace --doc
        check-jsonschema --check-metaschema schemas/*.schema.json
      '';
    };

    ravel-coverage = {
      description = "Enforce the maintained Rust line-coverage floor.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        cargo llvm-cov nextest --workspace --all-features --fail-under-lines 80
      '';
    };

    ravel-audit = {
      description = "Check dependency advisories, licences, bans, and sources.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        cargo deny check
      '';
    };

    ravel-verify-scaffold = {
      description = "Verify the repository scaffold contract.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        cargo run --locked -p ravel-xtask -- verify scaffold
      '';
    };

    ravel-verify-milestone = {
      description = "Verify one milestone evidence manifest.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        if [[ "$#" -ne 1 ]]; then
          echo "usage: ravel-verify-milestone M<N>" >&2
          exit 2
        fi
        cargo run --locked -p ravel-xtask -- verify milestone "$1"
      '';
    };

    ravel-verify-environment = {
      description = "Verify the complete devenv and Nix flake contract.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        devenv test --no-tui
        nix flake check --accept-flake-config
      '';
    };
  };

  tasks = {
    "ravel:fmt".exec = "ravel-format";
    "ravel:typecheck".exec = "ravel-typecheck";
    "ravel:lint".exec = "ravel-lint";
    "ravel:test".exec = "ravel-test";
    "ravel:coverage".exec = "ravel-coverage";
    "ravel:audit".exec = "ravel-audit";
    "ravel:verify-scaffold".exec = "ravel-verify-scaffold";
    "ravel:verify-environment".exec = "ravel-verify-environment";

    "ravel:verify" = {
      after = [
        "ravel:fmt"
        "ravel:typecheck"
        "ravel:lint"
        "ravel:test"
        "ravel:coverage"
        "ravel:audit"
        "ravel:verify-scaffold"
      ];
      exec = "true";
    };
  }
  // builtins.listToAttrs (
    map (milestone: {
      name = "ravel:verify-milestone:${lib.toLower milestone}";
      value.exec = "ravel-verify-milestone ${milestone}";
    }) milestones
  );

}
