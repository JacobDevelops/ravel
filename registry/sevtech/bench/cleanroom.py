#!/usr/bin/env python3
"""Prepare the optional exact-pack Cleanroom comparator; never launch Minecraft."""
import argparse
import json
from pathlib import Path, PurePosixPath
import shlex
import shutil
import stat
import subprocess
import tempfile
import tomllib
import urllib.request
import zipfile

import prepare

VERSION = "0.7.8-alpha"
URL = "https://github.com/CleanroomMC/Cleanroom/releases/download/0.7.8-alpha/cleanroom-0.7.8-alpha.zip"
SHA256 = "1d7c79c314d1bb590a0706bb9c9c8cc33412324a52035588df8b6614a74b8620"
GUIDE = "https://cleanroommc.com/wiki/end-user-guide/preparing-your-modpack"
EXCLUDED = {"saves", "logs", "crash-reports", "screenshots", "backups"}
WARNINGS = {
    "foamfix-": "FoamFix: upstream describes issues in the Cleanroom environment.",
    "mousetweaks-": "Mouse Tweaks: upstream describes outdated/problematic behavior.",
    "betterfps-": "BetterFPS: upstream says it breaks core functionality.",
    "baubles-": "Baubles: upstream labels it unmaintained, not a demonstrated runtime failure.",
    "forgelin-": "Forgelin: upstream labels it unmaintained, not a demonstrated runtime failure.",
}


def fetch_overlay(baseline):
    archive = baseline / ("cleanroom-" + VERSION + ".zip")
    prepare.require(not archive.is_symlink(), "overlay archive must not be linked")
    if not archive.exists():
        with tempfile.NamedTemporaryFile(dir=baseline, delete=False) as stream:
            temporary = Path(stream.name)
        try:
            urllib.request.urlretrieve(URL, temporary)
            prepare.require(prepare.digest(temporary) == SHA256, "Cleanroom download SHA256 differs")
            temporary.replace(archive)
        finally:
            temporary.unlink(missing_ok=True)
    prepare.require(prepare.digest(archive) == SHA256, "cached Cleanroom download SHA256 differs")
    return archive


def write_pins(baseline, archive, artifacts):
    values = {"schema_version": 1, "version": VERSION, "url": URL, "sha256": SHA256}
    path = baseline / "cleanroom-inputs.toml"
    mods = {name: value[1] for name, value in sorted(artifacts.items())}
    if path.exists():
        previous = tomllib.loads(path.read_text())
        prepare.require(all(previous.get(k) == v for k, v in values.items()) and
                        previous.get("mods") == mods, "Cleanroom input pins changed")
    content = "".join(k + " = " + json.dumps(v) + "\n" for k, v in values.items())
    content += "archive = " + json.dumps(str(archive.relative_to(baseline))) + "\n\n[mods]\n"
    content += "".join(json.dumps(k) + " = " + json.dumps(v) + "\n" for k, v in mods.items())
    temporary = path.with_suffix(".toml.tmp")
    temporary.write_text(content)
    temporary.replace(path)


def ignore_private(directory, names):
    ignored = []
    for name in names:
        path = Path(directory) / name
        if name in EXCLUDED and path.is_dir():
            ignored.append(name)
        else:
            prepare.require(not path.is_symlink(), "cannot clone linked original input: " + str(path))
    return ignored


def apply_overlay(archive, instance):
    with zipfile.ZipFile(archive) as source:
        pack = json.loads(source.read("mmc-pack.json"))
        components = {row["uid"]: row["version"] for row in pack["components"]}
        prepare.require(components == {"net.minecraft": "1.12.2", "org.lwjgl": "3.4.3",
                                       "net.minecraftforge": VERSION}, "unexpected overlay components")
        for entry in source.infolist():
            relative = PurePosixPath(entry.filename)
            prepare.require(not relative.is_absolute() and ".." not in relative.parts and
                            not stat.S_ISLNK(entry.external_attr >> 16), "unsafe Cleanroom archive")
            if relative.parts[0] not in ("patches", "libraries"):
                continue
            target = instance.joinpath(*relative.parts)
            prepare.require(not target.is_symlink(), "linked overlay target: " + str(target))
            if entry.is_dir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with source.open(entry) as incoming, target.open("wb") as outgoing:
                    shutil.copyfileobj(incoming, outgoing)
        prepare.write_json(instance / "mmc-pack.json", pack)
        return json.loads(source.read("patches/net.minecraftforge.json"))


def clone_instance(prism, archive, artifacts):
    original = prism / "instances/original"
    target = prism / "instances/cleanroom"
    prepare.require(original.is_dir() and not original.is_symlink(),
                    "original instance missing; run prepare.py first")
    prepare.verify_mods(original / ".minecraft", artifacts)
    prepare.require(not target.is_symlink(), "Cleanroom instance must not be linked")
    if target.exists():
        prepare.verify_mods(target / ".minecraft", artifacts)
        component = apply_overlay(archive, target)
        return target, component
    with tempfile.TemporaryDirectory(prefix=".cleanroom-", dir=original.parent) as temporary:
        stage = Path(temporary) / "cleanroom"
        shutil.copytree(original, stage, ignore=ignore_private)
        component = apply_overlay(archive, stage)
        prepare.verify_mods(stage / ".minecraft", artifacts)
        stage.rename(target)
    return target, component


def resolve_java(root):
    pin = tomllib.loads((root / "mise.toml").read_text())["tools"]["java"]
    prepare.require(isinstance(pin, str) and "27." in pin, "repository Java pin must select JDK 27")
    result = subprocess.run(["mise", "which", "java"], cwd=root, check=True,
                            capture_output=True, text=True)
    java = Path(result.stdout.strip()).resolve()
    prepare.require(java.is_file(), "mise Java executable missing")
    release = java.parent.parent / "release"
    values = dict(line.split("=", 1) for line in release.read_text().splitlines() if "=" in line)
    version = values.get("JAVA_VERSION", "").strip('"')
    prepare.require(version.startswith("27"), "mise which java did not resolve JDK 27; use mise exec")
    expected = pin.removeprefix("temurin-")
    runtime = values.get("JAVA_RUNTIME_VERSION", "").strip('"')
    expected_base, _, expected_build = expected.partition("+")
    actual_base, _, actual_build = runtime.partition("+")
    normalize = lambda value: tuple((value.split(".") + ["0", "0"])[:3])
    prepare.require(normalize(expected_base) == normalize(actual_base) and expected_build == actual_build,
                    "mise Java differs from repository pin")
    return str(java), pin, version


def configure_instance(instance, java, version, agent, output):
    prepare.require(":" not in str(output), "GC output path must not contain ':'")
    args = ["-javaagent:" + str(agent) + "=" + str(output),
            "-Xlog:gc*,safepoint:file=" + str(output / "gc.log") + ":time,uptime,level,tags"]
    prepare.update_ini(instance / "instance.cfg", {
        "name": "SevTech Ages 3.2.3 — Cleanroom " + VERSION,
        "InstanceType": "OneSix", "OverrideJavaLocation": "true", "JavaPath": java,
        "JavaVersion": version, "OverrideJavaArgs": "true", "JvmArgs": shlex.join(args),
        "AutomaticJava": "false", "IgnoreJavaCompatibility": "true",
        "OverrideMemory": "true", "MinMemAlloc": 4096, "MaxMemAlloc": 8192,
        "OverrideWindow": "true", "MinecraftWinWidth": 1280, "MinecraftWinHeight": 720,
        "LaunchMaximized": "false",
    })
    return args, prepare.configure_options(instance / ".minecraft")


def setup(root, baseline, agent, output):
    fixture = baseline.parent
    _, artifacts, _ = prepare.verify_inputs(root, fixture)
    java, pin, version = resolve_java(root)
    archive = fetch_overlay(baseline)
    write_pins(baseline, archive, artifacts)
    prism = baseline / "prism"
    instance, component = clone_instance(prism, archive, artifacts)
    jvm_args, options = configure_instance(instance, java, version, agent, output)
    warnings = [{"artifact": name, "upstream_note": note, "source": GUIDE}
                for name in sorted(artifacts) for prefix, note in WARNINGS.items()
                if name.lower().startswith(prefix)]
    return {
        "status": "prepared-not-launched", "optional": True, "instance": str(instance),
        "launch_method": "direct Java command resolved by launch.py",
        "java": java, "java_pin": pin, "java_version": version,
        "collector": "JDK 27 default G1; no Java 8 pack tuning flags", "jvm_args": jvm_args,
        "mod_count": len(artifacts), "mod_changes": [], "warnings": warnings,
        "component_java_majors": component.get("compatibleJavaMajors"),
        "java_compatibility_override": "Requested JDK 27; release component advertises only Java 25. "
                                       "Prism compatibility check explicitly disabled for this instance.",
        "observer": {"path": str(agent), "sha256": prepare.digest(agent),
                     "compatibility": "retained; Foundation runtime compatibility unverified"},
        "runtime_errors": [], "runtime_observed": False, "options": options,
        "installation_source": "https://github.com/CleanroomMC/Cleanroom/blob/0.7.8-alpha/README.md#client",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="observer and GC output directory")
    parser.add_argument("--agent", type=Path, required=True, help="existing observer agent JAR")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[3]
    baseline = root / "tests/fixtures/local" / prepare.FIXTURE / "baseline"
    output, agent = args.output.resolve(), args.agent.resolve()
    prepare.require(not any(c in str(agent) + str(output) for c in "\n\r="), "invalid agent/output path")
    output.mkdir(parents=True, exist_ok=True, mode=0o700)
    baseline.mkdir(parents=True, exist_ok=True, mode=0o700)
    try:
        prepare.require(agent.is_file(), "observer agent missing: " + str(agent))
        settings = setup(root, baseline, agent, output)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile, subprocess.SubprocessError) as error:
        settings = {"status": "setup-blocked", "optional": True, "blocker": str(error),
                    "runtime_observed": False, "mod_changes": []}
        prepare.write_json(output / "cleanroom-settings.json", settings)
        raise SystemExit("Cleanroom optional setup blocked: " + str(error)) from error
    prepare.write_json(output / "cleanroom-settings.json", settings)
    prepare.write_json(output / "settings.json", {
        "fixture": prepare.FIXTURE, "instance": "cleanroom", "options": settings["options"],
        "artifactCount": settings["mod_count"], "agentSha256": settings["observer"]["sha256"]})
    print(json.dumps(settings, indent=2))


if __name__ == "__main__":
    main()
