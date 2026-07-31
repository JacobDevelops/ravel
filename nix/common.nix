{
  config,
  lib,
  pkgs,
  ...
}:

let
  tracyWithFrameExport = pkgs.tracy.overrideAttrs (old: {
    patches = (old.patches or [ ]) ++ [
      ../tests/performance/collectors/tracy-csvexport-frames.patch
    ];
  });
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
      message = "unsupported Ravel host system";
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
    jdk25
    jq
    jujutsu
    markdownlint-cli
    nixfmt
    # Pure X clients for setup-only nested-display work. The nested X server and
    # its GL stack stay host-provided: a nixpkgs Xephyr or glxinfo would report
    # on nixpkgs' Mesa rather than the Mesa the game actually renders through.
    openbox
    pkg-config
    shellcheck
    shfmt
    statix
    taplo
    tracyWithFrameExport
    unzip
    xdotool
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
        check-jsonschema --schemafile schemas/fixture.schema.json \
          tests/fixtures/descriptors/atm11-0.2.1.json
        check-jsonschema --schemafile schemas/runtime-inputs.schema.json \
          tests/fixtures/descriptors/runtime-inputs-26.1.2.json
        check-jsonschema --schemafile schemas/performance-collector.schema.json \
          tests/performance/fixtures/collector-dry-run.json
        check-jsonschema --schemafile schemas/performance-result.schema.json \
          tests/performance/fixtures/result-dry-run.json

        duplicate_event_fixture="$(mktemp)"
        trap 'rm -f "$duplicate_event_fixture"' EXIT
        jq '.events += [(.events[] | select(.kind == "process-start"))]' \
          tests/performance/fixtures/collector-dry-run.json >"$duplicate_event_fixture"
        if check-jsonschema \
          --schemafile schemas/performance-collector.schema.json \
          "$duplicate_event_fixture"; then
          echo "collector schema accepted a duplicate singleton event" >&2
          exit 1
        fi
      '';
    };

    ravel-coverage = {
      description = "Enforce the maintained Rust line-coverage floor.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        # The per-gate spike candidates are benchmark-only instruments: what they
        # measure needs a JVM, a launcher, or a game, so they are excluded under
        # the roadmap's reviewed-allowlist clause. The shared harness is not
        # excluded — it is ordinary logic every gate depends on, so it earns the
        # floor like anything else. The fence in `ravel-verify-scaffold` keeps
        # both out of the production graph.
        cargo llvm-cov nextest --workspace --all-features --fail-under-lines 80 \
          --ignore-filename-regex '(^|/)experiments/adg-'
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

    ravel-verify-spikes = {
      description = "Verify recorded architecture spike results.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        cargo run --locked -p ravel-xtask -- verify spikes
      '';
    };

    ravel-spike-adg-1 = {
      description = "Measure the ADG-1 JVM entry candidates and record their evidence.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        if [[ ! -f tests/fixtures/local/tooling/minecraft-26.1.2/client.jar ]]; then
          echo "the pinned client jar is absent; see tests/fixtures/local/README.md" >&2
          exit 1
        fi

        # Release, because a debug boundary would measure the profile rather
        # than the mechanism.
        cargo build --locked --release -p ravel-spike-adg-1-jvm-entry
        ./target/release/ravel-spike-adg-1-jvm-entry
      '';
    };

    ravel-spike-adg-2 = {
      description = "Measure the ADG-2 mod-execution candidates and record their evidence.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        # Release, because a debug boundary would measure the profile rather
        # than the mechanism. The guest is built twice from one source: once
        # natively for the dynamic-library candidate, once for WebAssembly.
        cargo build --locked --release -p ravel-spike-adg-2-guest --target wasm32-unknown-unknown
        cargo build --locked --release -p ravel-spike-adg-2-guest -p ravel-spike-adg-2-mod-execution
        ./target/release/ravel-spike-adg-2-mod-execution
      '';
    };

    ravel-spike-adg-3 = {
      description = "Generate the pinned 26.1.2 binding both ways and record the evidence.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        if [[ ! -f tests/fixtures/local/tooling/minecraft-26.1.2/client.jar ]]; then
          echo "the pinned client jar is absent; see tests/fixtures/local/README.md" >&2
          exit 1
        fi

        cargo build --locked --release -p ravel-spike-adg-3-bindings
        ./target/release/ravel-spike-adg-3-bindings
      '';
    };

    ravel-spike-adg-4 = {
      description = "Measure the ADG-4 state-ownership models and record their evidence.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        # Release, because a debug boundary would measure the profile rather
        # than the mechanism.
        cargo build --locked --release -p ravel-spike-adg-4-state
        ./target/release/ravel-spike-adg-4-state
      '';
    };

    ravel-spike-adg-5 = {
      description = "Check the pinned launcher's configuration surface without launching it.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        if [[ ! -f tests/fixtures/local/tooling/prism-11.0.3/PrismLauncher-Linux-x86_64.AppImage ]]; then
          echo "the pinned launcher is absent; see tests/fixtures/local/README.md" >&2
          exit 1
        fi

        cargo build --locked --release -p ravel-spike-adg-5-prism
        ./target/release/ravel-spike-adg-5-prism
      '';
    };

    ravel-verify-stack = {
      description = "Verify the jj change stack contract.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        if [[ ! -d .jj ]]; then
          echo "no jj repository here; skipping the stack contract"
          exit 0
        fi

        conventional='^(feat|fix|docs|refactor|perf|test|chore|build|ci|revert)(\([a-z0-9-]+\))?!?: .'
        fixup='(?i)(\bwip\b|fixup|squash!|address (review|feedback)|review comments)'
        status=0

        report() {
          local found
          found=$(jj log --no-pager --no-graph -r "$1" \
            -T 'change_id.short() ++ " " ++ description.first_line() ++ "\n"')
          if [[ -n "$found" ]]; then
            printf '%s\n%s\n' "$2" "$found" >&2
            status=1
          fi
        }

        if [[ "$(jj config get git.write-change-id-header 2>/dev/null || true)" != "true" ]]; then
          echo "change ids are not written to git commits;" \
            "run: jj config set --repo git.write-change-id-header true" >&2
          status=1
        fi

        report "::@ & mutable() & ~(@ & empty()) & ~description(regex:'$conventional')" \
          "describe each change up front, planned empty ones included (jj describe -m ...):"
        report "::@ & mutable() & description(regex:'$fixup')" \
          "revise the change that is wrong instead of stacking a follow-up (jj squash --into <change>):"
        report "::@ & mutable() & conflicts()" \
          "resolve conflicts inside the change that carries them (jj resolve):"

        exit "$status"
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

    ravel-fixture-reconstruct = {
      description = "Reconstruct the pinned fixture under the ignored local boundary.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        cargo run --locked -p ravel-cli -- fixture reconstruct
      '';
    };

    ravel-fixture-verify = {
      description = "Verify pinned fixture versions, counts, and hashes.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        cargo run --locked -p ravel-cli -- fixture verify
      '';
    };

    ravel-baseline = {
      description = "Validate a named performance profile and its dry-run contract.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        if [[ "$#" -ne 2 || "$1" != "initial" || "$2" != "dry-run" ]]; then
          echo "usage: ravel-baseline initial dry-run" >&2
          echo "native capture remains an operator-run procedure" >&2
          exit 2
        fi

        taplo format --check tests/performance/profiles/initial.toml
        check-jsonschema --schemafile schemas/performance-collector.schema.json \
          tests/performance/fixtures/collector-dry-run.json
        check-jsonschema --schemafile schemas/performance-result.schema.json \
          tests/performance/fixtures/result-dry-run.json
        jq --exit-status \
          '.captureMode == "dry-run" and .profile.id == "initial" and .profile.version == 3' \
          tests/performance/fixtures/collector-dry-run.json >/dev/null
        jq --exit-status \
          '.captureMode == "dry-run" and .profile.id == "initial" and .profile.version == 3' \
          tests/performance/fixtures/result-dry-run.json >/dev/null
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
    "ravel:verify-spikes".exec = "ravel-verify-spikes";
    "ravel:spike-adg-1".exec = "ravel-spike-adg-1";
    "ravel:spike-adg-2".exec = "ravel-spike-adg-2";
    "ravel:spike-adg-3".exec = "ravel-spike-adg-3";
    "ravel:spike-adg-4".exec = "ravel-spike-adg-4";
    "ravel:spike-adg-5".exec = "ravel-spike-adg-5";
    "ravel:verify-stack".exec = "ravel-verify-stack";
    "ravel:verify-environment".exec = "ravel-verify-environment";
    "ravel:fixture-reconstruct".exec = "ravel-fixture-reconstruct";
    "ravel:fixture-verify".exec = "ravel-fixture-verify";
    "ravel:baseline".exec = "ravel-baseline initial dry-run";

    "ravel:verify" = {
      after = [
        "ravel:fmt"
        "ravel:typecheck"
        "ravel:lint"
        "ravel:test"
        "ravel:coverage"
        "ravel:audit"
        "ravel:verify-scaffold"
        "ravel:verify-spikes"
        "ravel:verify-stack"
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
