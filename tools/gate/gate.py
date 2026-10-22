#!/usr/bin/env python3
"""Scoped lane checks and content-addressed landing receipts."""

import datetime
import hashlib
from functools import cache
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[2]
LEGACY_PATHS = (
    "jvm/", "minecraft/versions/1.12.2/", "crates/ravel-render",
    "crates/ravel-minecraft", "crates/ravel-bootstrap-runtime/",
    "crates/ravel-composed-host/", "tools/ravel-xtask/",
    "Cargo.toml", "Cargo.lock", "Makefile.toml", "rust-toolchain.toml", ".cargo/", "tools/gate/",
)


def run(*args, **kwargs):
    print("+ " + " ".join(map(str, args)), flush=True)
    return subprocess.run(args, check=True, **kwargs)


def capture(*args):
    return subprocess.check_output(args, text=True).strip()


def changed_paths(base=None):
    base = base if base is not None else os.environ.get("RAVEL_GATE_BASE", "fork_point(@ | main)")
    return capture("jj", "diff", "--from", base, "--to", "@", "--name-only").splitlines()


@cache
def cargo_metadata():
    return json.loads(capture("cargo", "metadata", "--format-version", "1", "--locked", "--all-features"))


def legacy_changed(paths):
    if any(p.startswith(LEGACY_PATHS) for p in paths):
        return True
    metadata = cargo_metadata()
    packages = {p["id"]: p for p in metadata["packages"] if p["id"] in metadata["workspace_members"]}
    dependencies = {node["id"]: [dep["pkg"] for dep in node["deps"]
                                if any(kind["kind"] in (None, "build") for kind in dep["dep_kinds"])]
                    for node in metadata["resolve"]["nodes"]}
    closure = {key for key, p in packages.items() if p["name"] == "ravel-composed-host"}
    while True:
        expanded = closure | {dep for key in closure for dep in dependencies[key] if dep in packages}
        if expanded == closure:
            break
        closure = expanded
    roots = tuple(Path(packages[key]["manifest_path"]).parent.relative_to(ROOT).as_posix() + "/"
                  for key in closure)
    return any(p.startswith(roots) for p in paths)


def package_scope(paths):
    metadata = cargo_metadata()
    members = set(metadata["workspace_members"])
    packages = {p["id"]: p for p in metadata["packages"] if p["id"] in members}
    roots = {key: Path(p["manifest_path"]).parent.relative_to(ROOT).as_posix() + "/"
             for key, p in packages.items()}
    global_paths = ("Cargo.toml", "Cargo.lock", "Makefile.toml", "rust-toolchain.toml", "clippy.toml")
    inputs = (".cargo/", ".config/", "tools/gate/", "minecraft/", "schemas/", "jvm/", "evidence/", "tests/fixtures/")
    all_packages = any(p in global_paths or p.startswith(inputs) for p in paths)
    touched = set(packages) if all_packages else {
        key for key, root in roots.items() if any(p.startswith(root) for p in paths)
    }
    affected = set(touched)
    while True:
        expanded = affected | {node["id"] for node in metadata["resolve"]["nodes"]
                               if node["id"] in members and any(
                                   dep in affected for dep in node["dependencies"])}
        if expanded == affected:
            break
        affected = expanded
    names = lambda ids: sorted(packages[key]["name"] for key in ids)
    return names(touched), names(affected)


def quick(paths):
    run("cargo", "make", "ravel:fmt")
    touched, affected = package_scope(paths)
    print(f"gate scope: clippy={','.join(touched) or 'none'} tests={','.join(affected) or 'none'}", flush=True)
    if touched:
        flags = [arg for name in touched for arg in ("-p", name)]
        run("cargo", "clippy", "--locked", "--all-targets", "--all-features", *flags, "--", "-D", "warnings")
    if affected:
        run("cargo", "make", "ravel:prepare-normalizer")
        flags = [arg for name in affected for arg in ("-p", name)]
        tested = False
        for directory in sorted(Path("minecraft/versions").iterdir()):
            if not directory.is_dir():
                continue
            version = directory.name
            if capture("cargo", "run", "--quiet", "--locked", "-p", "ravel-xtask", "--", "version-matrix", version) != "live":
                continue
            env = dict(os.environ, RAVEL_MINECRAFT_VERSION=version,
                       CARGO_TARGET_DIR=str(ROOT / "target/matrix" / version))
            tested = True
            run("cargo", "nextest", "run", "--locked", "--all-features", *flags, env=env)
            if "ravel-composed-host" in affected:
                run("cargo", "nextest", "run", "--locked", "-p", "ravel-composed-host", "--no-default-features", env=env)
        if not tested:
            raise RuntimeError("no tracked live version was testable; scoped tests proved nothing")
        run("cargo", "test", "--locked", "--doc", "--all-features", *flags)
        if "ravel-bootstrap-runtime" in affected:
            run("cargo", "test", "--locked", "-p", "ravel-bootstrap-runtime", "--all-features")
    run("cargo", "run", "--quiet", "--locked", "-p", "ravel-xtask", "--", "package", "digests", "--check", "--all")
    for suite in ("tools/gate", "jvm/legacy-platform/tools", "registry/sevtech", "registry/sevtech/bench"):
        run("python3", "-B", "-m", "unittest", "discover", "-s", suite, "-p", "test_*.py")
    if any(p.startswith("registry/") for p in paths):
        run("cargo", "make", "ravel:registry-verify")


def tree_identity():
    return hashlib.sha256(subprocess.check_output(("jj", "debug", "tree", "-r", "@"))).hexdigest()


def matrix_eligibility():
    return [[directory.name, capture("cargo", "run", "--quiet", "--locked", "-p", "ravel-xtask",
                                     "--", "version-matrix", directory.name)]
            for directory in sorted((ROOT / "minecraft/versions").iterdir()) if directory.is_dir()]


def legacy_inputs():
    platform = ROOT / "tests/fixtures/local/legacy-platform"
    paths = {str(platform / line.split()[1]) for line in
             (ROOT / "jvm/legacy-platform/dependencies.sha256").read_text().splitlines()
             if line.strip() and not line.startswith("#")}
    for name in ("client-classpath.txt", "server-classpath.txt"):
        path = platform / name
        paths.add(str(path))
        if path.is_file():
            paths.update(line for line in path.read_text().splitlines() if line)
    directory = platform / "natives"
    paths.add(str(directory))
    paths.update(str(path) for path in directory.rglob("*") if path.is_file())
    return paths


def input_digests():
    paths = {entry["path"] for declaration in (ROOT / "minecraft/versions").glob("*/adapter.toml")
             for entry in tomllib.loads(declaration.read_text()).get("inputs", {}).values()}
    return digest_paths(paths)


def digest_paths(paths):
    digests = {}
    for path in sorted(paths):
        source = ROOT / path
        if source.is_file():
            with source.open("rb") as stream:
                digests[path] = hashlib.file_digest(stream, "sha256").hexdigest()
        else:
            digests[path] = None
    return digests


def listing_digest(directory):
    directory = Path(directory)
    entries = []
    visited = set()
    for root, directories, files in os.walk(directory, followlinks=True):
        current = Path(root)
        if current.resolve() in visited:
            directories.clear()
            continue
        visited.add(current.resolve())
        for name in sorted(directories + files):
            path = current / name
            stat = path.stat() if path.exists() else path.lstat()
            entries.append((path.relative_to(directory).as_posix(),
                            None if path.is_dir() else (stat.st_size, stat.st_mtime_ns),
                            str(path.readlink()) if path.is_symlink() else None))
    entries.sort()
    return hashlib.sha256(json.dumps([directory.is_dir(), entries]).encode()).hexdigest()


def receipt_context():
    environment = {key: value for key, value in os.environ.items()
                   if (key.startswith(("RAVEL_", "CARGO_", "RUST", "NEXTEST_", "JAVA_", "JDK_", "PYTHON", "LLVM_"))
                       or key in ("PATH", "HOME", "CC", "CXX", "AR", "JAVA", "JAVAC", "JAR",
                                  "ALSOFT_DRIVERS", "FIXTURE_ID"))
                   and key != "RAVEL_FORCE_VERIFY" and not key.startswith(("CARGO_MAKE", "CARGO_TERM_"))
                   and key not in ("RUST_RECURSION_COUNT", "RUSTUP_TOOLCHAIN_SOURCE", "RUSTUP_TOOLCHAIN")}
    source = os.environ.get("RAVEL_CLEAN_ROOM_SRC") or str(ROOT / "tests/fixtures/local/m6-clean-room/src")
    manifest = os.environ.get("RAVEL_CLEAN_ROOM_MANIFEST") or str(ROOT / "tests/fixtures/local/m6-clean-room/fetch-manifest.txt")
    return {"environment": hashlib.sha256(json.dumps(environment, sort_keys=True).encode()).hexdigest(),
            "clean_room": listing_digest(source), "manifest": digest_paths([manifest]),
            "rustc": capture("rustc", "-vV")}


def receipt_key(tree, eligibility, inputs, context, legacy):
    return hashlib.sha256(json.dumps([tree, eligibility, inputs, context, legacy], sort_keys=True).encode()).hexdigest()


def legacy_live():
    run("cargo", "make", "ravel:legacy-agent-build")
    run("cargo", "make", "ravel:legacy-agent-live")


def land():
    tree = tree_identity()
    eligibility = matrix_eligibility()
    inputs = input_digests()
    context = receipt_context()
    legacy = legacy_changed(changed_paths("trunk()"))
    stamp = ROOT / "target/gate/verified" / receipt_key(tree, eligibility, inputs, context, legacy)
    try:
        previous = json.loads(stamp.read_text())
    except (FileNotFoundError, json.JSONDecodeError):
        previous = None
    if (previous and previous.get("eligibility") == eligibility
            and previous.get("inputs") == inputs
            and previous.get("legacy_inputs") == digest_paths(legacy_inputs())
            and os.environ.get("RAVEL_FORCE_VERIFY") != "1"):
        run("cargo", "make", "ravel:verify-stack")
        print(f"already verified tree {tree} at {previous['time']}")
        return
    run("cargo", "make", "ravel:verify")
    if legacy:
        legacy_live()
    if (tree_identity() != tree or matrix_eligibility() != eligibility
            or input_digests() != inputs or receipt_context() != context):
        raise RuntimeError("tree or matrix inputs changed during verification; no receipt written")
    stamp.parent.mkdir(parents=True, exist_ok=True)
    receipt = {"time": datetime.datetime.now(datetime.timezone.utc).isoformat(),
               "eligibility": eligibility, "inputs": inputs,
               "legacy_inputs": digest_paths(legacy_inputs())}
    temporary = stamp.with_suffix(f".{os.getpid()}.tmp")
    temporary.write_text(json.dumps(receipt) + "\n")
    os.replace(temporary, stamp)


def main():
    os.chdir(ROOT)
    mode = sys.argv[1]
    if mode == "land":
        land()
        return
    paths = changed_paths()
    legacy = legacy_changed(paths)
    if mode == "scope":
        print(json.dumps({"paths": paths, "packages": package_scope(paths), "legacy": legacy}))
        return
    quick(paths)
    if mode == "lane":
        if legacy:
            print("legacy live: triggered by JVM, legacy, renderer or hook/generation changes", flush=True)
            legacy_live()
        else:
            print("legacy live: skipped; no exercised paths changed", flush=True)


if __name__ == "__main__":
    main()
