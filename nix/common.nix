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

        # Every walk below prunes `tests/fixtures/local/`, and a prune rule that
        # widens by a directory stops covering the public fixtures beside it
        # without saying so. This is the gate on that: the public fixtures are
        # repository content and have to reach the formatter.
        require_public_fixtures() {
          local kind="$1"
          shift
          local missing
          missing="$(
            comm -23 \
              <(find ./tests/fixtures/public -type f -name "$kind" | sort) \
              <(printf '%s\n' "$@" | sort)
          )"
          if [ -n "$missing" ]; then
            echo "the $kind exclusion reaches past tests/fixtures/local and drops:" >&2
            echo "$missing" >&2
            exit 1
          fi
        }

        # Restricted local material is ignored rather than repository content, so
        # it is not held to the repository's formatting. Its README is tracked and
        # stays in.
        mapfile -d "" toml_files < <(
          find . -type f -name "*.toml" \
            -not -path "./.devenv/*" \
            -not -path "./.direnv/*" \
            -not -path "./.jj/*" \
            -not -path "./target/*" \
            -not -path "./tests/fixtures/local/*" \
            -print0
        )
        require_public_fixtures "*.toml" "''${toml_files[@]}"
        taplo format --check "''${toml_files[@]}"

        mapfile -d "" markdown_files < <(
          find . -type f -name "*.md" \
            -not -path "./.devenv/*" \
            -not -path "./.direnv/*" \
            -not -path "./.jj/*" \
            -not -path "./target/*" \
            -not \( -path "./tests/fixtures/local/*" \
              ! -path "./tests/fixtures/local/README.md" \) \
            -print0
        )
        require_public_fixtures "*.md" "''${markdown_files[@]}"
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
        # nextest gives every test its own process, so state that leaks between
        # tests through a process global is invisible to it by construction. The
        # plain runner shares one process across threads, and ravel-bootstrap is
        # the crate that holds such globals.
        #
        # This step failed once, on 2026-08-08, with no `test result:` line, no
        # named test and no panic — the signature of the binary dying rather than
        # a test asserting. It did not reproduce in roughly 53 further runs,
        # isolated, under concurrent build load, or at --test-threads=1, so it is
        # disclosed here rather than fixed speculatively: a fix for a cause
        # nobody has established is a guess that later reads as a guarantee. If
        # it recurs, capture the exit status first — 134 is SIGABRT, 139 SIGSEGV,
        # 137 an OOM kill worth checking dmesg for.
        cargo test -p ravel-bootstrap --all-features
        cargo test --workspace --doc
        check-jsonschema --check-metaschema schemas/*.schema.json
        check-jsonschema --schemafile schemas/fixture.schema.json \
          tests/fixtures/descriptors/atm10-tts-2.0.2.json
        check-jsonschema --schemafile schemas/runtime-inputs.schema.json \
          tests/fixtures/descriptors/runtime-inputs-1.21.1.json
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

    # `cargo test` compiles the pack example and runs nothing, so until this script
    # existed a change that made the documented example refuse validation left every
    # gate green. The example is the artifact M4-W5's Output column names, so running
    # it is part of the gate rather than something a reviewer does by hand.
    ravel-verify-example-pack = {
      description = "Run the pack example and check the pack it writes.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        directory="$(mktemp -d)"
        trap 'rm -rf "$directory"' EXIT

        # The example refuses with a nonzero status when its pack does not hold
        # together, so `set -e` is what turns a broken example into a failed gate.
        reported="$(cargo run --locked -q -p ravel-pack --example example_pack -- "$directory")"

        # Two comparisons rather than one. The expected list is written out here so
        # the gate has an opinion the example cannot move, and the example's own
        # report is compared to the tree as well, because a report that disagrees
        # with what `write` emitted is the failure a single comparison would miss.
        expected="$(printf '%s\n' \
          assets/copper_mod/lang/en_us.json \
          assets/example_pack/textures/gui/progress.png \
          assets/lantern_mod/lang/en_us.json \
          data/c/tags/item/ingots.json \
          data/example_pack/recipe/copper_lantern.json \
          ravel/config/lantern_mod.json \
          ravel/pack.json | sort)"
        written="$(cd "$directory" && find . -type f -printf '%P\n' | sort)"
        announced="$(echo "$reported" | sed -n 's/^  //p' | sort)"

        if [ "$written" != "$expected" ]; then
          echo "the pack example wrote a tree this gate does not expect" >&2
          diff <(echo "$expected") <(echo "$written") >&2 || true
          exit 1
        fi
        if [ "$announced" != "$written" ]; then
          echo "the pack example reported files it did not write, or the reverse" >&2
          diff <(echo "$announced") <(echo "$written") >&2 || true
          exit 1
        fi

        # `ravel/pack.json` is the reproducible manifest `SPEC.md` §4.3 asks the pack
        # author to produce, so its absence is a different failure from a missing
        # data file and is named separately.
        if ! jq -e '.members | length > 0' "$directory/ravel/pack.json" >/dev/null; then
          echo "the written pack manifest names no member" >&2
          exit 1
        fi
      '';
    };

    # `examples/minimal-mod` is the package an author is told to copy, and a
    # digest in it that no longer matches the file beside it teaches every author
    # that a stale manifest is normal. `--check` writes nothing, so the gate
    # reports the drift rather than papering over it.
    ravel-verify-example-package = {
      description = "Check the example package's manifest digests.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        cargo run --locked -q -p ravel-xtask -- package digests --check examples/minimal-mod
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

    # The only check in the graph that starts a JVM, and the only one that runs
    # `premain` at all. Everything else here tests Rust that a JVM would call;
    # this tests the path the JVM actually takes to reach it, which is where a
    # launch fails when it fails. M3-W7 found that path broken by a null
    # `System.getProperty` reaching a native method — every launch aborted, and
    # no gate noticed, because this script existed and nothing ran it.
    #
    # It builds rather than requiring artifacts, because a check that fails on a
    # clean checkout with instructions is a check nobody runs.
    ravel-verify-jvm = {
      description = "Run Ravel's agent, transformer and native library against a real JVM.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"
        cargo build --locked -p ravel-minecraft-adapter
        cargo build --locked -p ravel-composed-host --release
        ./jvm/build/build-agent.sh
        ./jvm/harness/run-harness.sh
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

    ravel-measure-loader-overhead = {
      description = "Measure loader overhead and set the M2 regression budget.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        # Release, because a debug build would measure the profile rather than
        # the loader, and because the pack figures describe what an operator
        # would actually build.
        cargo build --locked --release -p ravel-spike-m2-loader-overhead
        ./target/release/ravel-spike-m2-loader-overhead
      '';
    };

    ravel-measure-profiling-overhead = {
      description = "Measure what opt-in profiling costs when it is off and record its budget.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        # Release, because the quantity measured is a branch that is not taken
        # and a debug build would measure the profile rather than the branch.
        cargo build --locked --release -p ravel-spike-m3-profiling-overhead
        ./target/release/ravel-spike-m3-profiling-overhead
      '';
    };

    ravel-measure-outward-crossing = {
      description = "Measure the three ways a mod's game action could reach the running game.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        # Release, because the quantity measured is a boundary crossing and a
        # debug build would measure the profile rather than the crossing.
        cargo build --locked --release -p ravel-spike-m6-outward-crossing
        ./target/release/ravel-spike-m6-outward-crossing
      '';
    };

    ravel-verify-budget = {
      description = "Compare the recorded runs against the regression budget.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        # Every recorded run at once, because the budget spans more than one
        # record and no single run measures all of it. A limit that no run
        # measured is a violation, which is what keeps a metric from being
        # dropped and read as having passed.
        cargo run --locked -p ravel-xtask -- verify budget \
          evidence/m2/loader-overhead.json \
          evidence/m3/profiling-overhead.json
      '';
    };

    ravel-measure-pack-compile = {
      description = "Measure what a pack-sized set of realistic mods costs to compile.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        # Release, because the figures describe what a player's machine would
        # actually build, and the generated packs are built in release too.
        cargo build --locked --release -p ravel-spike-m3-pack-compile
        ./target/release/ravel-spike-m3-pack-compile
      '';
    };

    ravel-measure-pack-compile-variants = {
      description = "Measure the four-core, dependency-graph, and link-time pack compile variants.";
      exec = ''
        set -euo pipefail
        cd "$DEVENV_ROOT"

        # Splices into the record the curve wrote, so that record has to exist
        # and its numbers are left exactly as they were measured.
        cargo build --locked --release -p ravel-spike-m3-pack-compile
        ./target/release/ravel-spike-m3-pack-compile --variants
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
    "ravel:verify-example-pack".exec = "ravel-verify-example-pack";
    "ravel:verify-example-package".exec = "ravel-verify-example-package";
    "ravel:verify-spikes".exec = "ravel-verify-spikes";
    "ravel:verify-jvm".exec = "ravel-verify-jvm";
    "ravel:spike-adg-1".exec = "ravel-spike-adg-1";
    "ravel:spike-adg-2".exec = "ravel-spike-adg-2";
    "ravel:spike-adg-3".exec = "ravel-spike-adg-3";
    "ravel:spike-adg-4".exec = "ravel-spike-adg-4";
    "ravel:spike-adg-5".exec = "ravel-spike-adg-5";
    "ravel:measure-loader-overhead".exec = "ravel-measure-loader-overhead";
    "ravel:measure-pack-compile".exec = "ravel-measure-pack-compile";
    "ravel:measure-pack-compile-variants".exec = "ravel-measure-pack-compile-variants";
    "ravel:measure-profiling-overhead".exec = "ravel-measure-profiling-overhead";
    "ravel:measure-outward-crossing".exec = "ravel-measure-outward-crossing";
    "ravel:verify-budget".exec = "ravel-verify-budget";
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
        "ravel:verify-example-pack"
        "ravel:verify-example-package"
        "ravel:verify-spikes"
        "ravel:verify-jvm"
        "ravel:verify-budget"
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
