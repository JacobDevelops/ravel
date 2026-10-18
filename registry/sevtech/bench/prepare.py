#!/usr/bin/env python3
"""Prepare the byte-pinned original pack in a private Prism data directory."""
import argparse
import configparser
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import shlex
import shutil
import stat
import subprocess
import sys
import tempfile
import tomllib
import zipfile

FIXTURE = "sevtech-ages-3.2.3"
JAVA = "java@temurin-8.0.504+1"
ARGS_SOURCE = "https://github.com/DarkPacks/SevTech-Ages/wiki/Recommended-Java-Args"
PACK_ARGS = [
    "-XX:+UseG1GC", "-Dsun.rmi.dgc.server.gcInterval=2147483646",
    "-XX:+UnlockExperimentalVMOptions", "-XX:G1NewSizePercent=20",
    "-XX:G1ReservePercent=20", "-XX:MaxGCPauseMillis=50",
    "-XX:G1HeapRegionSize=32M",
]
COMPONENTS = [
    {"uid": "net.minecraft", "version": "1.12.2", "important": True},
    {"uid": "net.minecraftforge", "version": "14.23.5.2860", "important": True},
    {"uid": "org.lwjgl", "version": "2.9.4-nightly-20150209"},
]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path, algorithm="sha256"):
    value = hashlib.new(algorithm)
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def resolve_input(root, fixture, raw):
    path = (root / raw.replace("${FIXTURE_ID}", FIXTURE)).resolve()
    require(path.is_relative_to(fixture.resolve()), "input escapes fixture: " + raw)
    require(path.is_file(), "missing acquired input: " + str(path))
    return path


def verify_inputs(root, fixture):
    ledger = root / "registry/inventory" / FIXTURE / "references.json"
    refs = json.loads(ledger.read_text())
    require(refs["fixture"]["id"] == FIXTURE, "wrong fixture ledger")
    acquired, bundled = refs["acquiredArtifacts"], refs["bundledArtifacts"]
    require((len(acquired), len(bundled)) == (254, 5), "expected 254 + 5 artifacts")
    archive = resolve_input(root, fixture, refs["fixture"]["archive"]["path"])
    require(digest(archive) == refs["fixture"]["archive"]["sha256"], "archive SHA256 differs")
    manifest_path = resolve_input(root, fixture, refs["fixture"]["manifest"]["path"])
    require(digest(manifest_path) == refs["fixture"]["manifest"]["sha256"], "manifest SHA256 differs")
    manifest = json.loads(manifest_path.read_text())
    pairs = lambda rows: sorted((row["projectId"], row["fileId"]) for row in rows)
    expected = pairs(refs["references"])
    require(len(set(expected)) == 254 and pairs(acquired) == expected,
            "acquired reference identities differ")
    require(sorted((r["projectID"], r["fileID"]) for r in manifest["files"]) == expected,
            "manifest reference identities differ")
    require(refs["fixture"]["minecraft"] == "1.12.2" and
            refs["fixture"]["modLoader"] == "forge-14.23.5.2860", "platform pins differ")
    artifacts = {}
    pins = {"archive": digest(archive), "manifest": digest(manifest_path)}
    with zipfile.ZipFile(archive) as source:
        require(json.loads(source.read("manifest.json")) == manifest, "archive manifest differs")
        entries = {p for p in source.namelist() if p.startswith("overrides/mods/") and p.endswith(".jar")}
        require(entries == {r["archiveEntry"] for r in bundled}, "bundled ledger differs")
        for row in acquired + bundled:
            name = row["artifact"]
            require(Path(name).name == name and name.endswith(".jar") and name not in artifacts,
                    "invalid or duplicate artifact name: " + name)
            path = resolve_input(root, fixture, row["path"])
            require(row["status"] == "fetched" and path.name == name and
                    path.stat().st_size == row["size"] and digest(path) == row["sha256"] and
                    digest(path, "sha1") == row["sha1"], "artifact bytes differ: " + name)
            if "archiveEntry" in row:
                require(hashlib.sha256(source.read(row["archiveEntry"])).hexdigest() == row["sha256"],
                        "archive bundled bytes differ: " + name)
            artifacts[name] = (path, row["sha256"])
            pins["mods/" + name] = row["sha256"]
    return archive, artifacts, pins


def verify_mods(game, artifacts):
    mods = game / "mods"
    require(mods.is_dir() and not mods.is_symlink(), "instance mods directory is missing or linked")
    entries = {p.name for p in mods.iterdir()}
    generated = {"memory_repo", "1.12.2"}
    require(entries - generated == set(artifacts), "existing instance mods population differs")
    for name in entries & generated:
        verify_extracted_repository(mods / name, artifacts)
    for name, (_, expected) in artifacts.items():
        path = mods / name
        require(path.is_file() and not path.is_symlink() and digest(path) == expected,
                "existing instance mod differs: " + name)


def verify_extracted_repository(repository, artifacts):
    """Forge extracts ContainedDeps to memory_repo or its legacy version directory."""
    require(repository.is_dir() and not repository.is_symlink(), "invalid Forge extracted repository")
    expected = {".jar": set(), ".meta": set()}
    for source, _ in artifacts.values():
        with zipfile.ZipFile(source) as host:
            for name in host.namelist():
                if not name.endswith(".jar"):
                    continue
                data = host.read(name)
                expected[".jar"].add(hashlib.sha256(data).hexdigest())
                if name + ".meta" in host.namelist():
                    manifest = host.read(name + ".meta")
                else:
                    with zipfile.ZipFile(io.BytesIO(data)) as nested:
                        entry = next((n for n in nested.namelist() if n.upper() == "META-INF/MANIFEST.MF"), None)
                        manifest = nested.read(entry) if entry else None
                if manifest is not None:
                    expected[".meta"].add(hashlib.sha256(manifest).hexdigest())
    seen = set()
    for path in repository.rglob("*"):
        require(not path.is_symlink(), "linked Forge memory repository entry")
        if path.is_dir():
            continue
        value = digest(path)
        require(path.suffix in expected and value in expected[path.suffix],
                "Forge extracted dependency differs: " + path.name)
        require((path.suffix, value) not in seen, "duplicate Forge extracted dependency")
        seen.add((path.suffix, value))


def archive_overrides(source):
    for entry in source.infolist():
        if not entry.filename.startswith("overrides/"):
            continue
        relative = PurePosixPath(entry.filename).relative_to("overrides")
        require(not relative.is_absolute() and ".." not in relative.parts and
                not stat.S_ISLNK(entry.external_attr >> 16), "unsafe archive override")
        if relative.parts:
            yield entry, relative


def unpack_overrides(archive, game, preserve=()):
    with zipfile.ZipFile(archive) as source:
        for entry, relative in archive_overrides(source):
            if relative.parts[0] in preserve:
                continue
            target = game.joinpath(*relative.parts)
            if entry.is_dir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with source.open(entry) as incoming, target.open("wb") as outgoing:
                    shutil.copyfileobj(incoming, outgoing)
        publisher = "overrides/options.txt"
        baseline = source.read(publisher) if publisher in source.namelist() else b""
        snapshot = game / ".benchmark-publisher-options.txt"
        require(not snapshot.is_symlink(), "linked publisher options baseline")
        snapshot.write_bytes(baseline)


def reset_overrides(archive, game):
    require(game.is_dir() and not game.is_symlink(), "instance game directory must be private")
    preserve = {"mods", "saves"}
    with zipfile.ZipFile(archive) as source:
        names = {relative.parts[0] for _, relative in archive_overrides(source)} - preserve
    for name in sorted(names):
        target = game / name
        if target.is_symlink() or target.is_file():
            target.unlink()
        elif target.exists():
            require(target.is_dir(), "invalid publisher override target: " + name)
            shutil.rmtree(target)
    unpack_overrides(archive, game, preserve)


def prepare_instance(prism, archive, artifacts, name="original"):
    instances = prism / "instances"
    instances.mkdir(parents=True, exist_ok=True)
    require(not instances.is_symlink(), "instance directory must not be a symlink")
    instance = instances / name
    require(not instance.is_symlink(), "instance must not be a symlink")
    if instance.exists():
        game = instance / ".minecraft"
        require(game.is_dir() and not game.is_symlink(), "instance game directory must be private")
        verify_mods(game, artifacts)
        reset_overrides(archive, game)
        return instance
    with tempfile.TemporaryDirectory(prefix="." + name + "-", dir=instances) as temporary:
        stage = Path(temporary) / name
        game = stage / ".minecraft"
        game.mkdir(parents=True)
        unpack_overrides(archive, game)
        (game / "mods").mkdir(exist_ok=True)
        for name, (source, _) in artifacts.items():
            shutil.copyfile(source, game / "mods" / name)
        verify_mods(game, artifacts)
        stage.rename(instance)
    return instance


def configure_caches(prism, cache_root):
    require(prism.is_dir() and not prism.is_symlink(), "Prism root must be private")
    for name in ("libraries", "assets", "meta"):
        target = prism / name
        source = target.resolve() if target.is_symlink() else cache_root / name
        if target.is_symlink() or not target.exists():
            if source.is_dir():
                require(source.resolve() != target.absolute(), "cache source points to itself")
                with tempfile.TemporaryDirectory(prefix=".cache-", dir=prism) as temporary:
                    stage = Path(temporary) / name
                    shutil.copytree(source.resolve(), stage)
                    if target.is_symlink():
                        target.unlink()
                    stage.rename(target)
            else:
                require(not target.is_symlink(), "unavailable shared cache: " + name)
                target.mkdir()
        require(target.is_dir() and not target.is_symlink(), "unavailable private cache: " + name)
        require(all(not path.is_symlink() for path in target.rglob("*")),
                "private cache contains an external link: " + name)


def update_ini(path, values):
    config = configparser.ConfigParser(interpolation=None)
    config.optionxform = str
    if path.exists():
        config.read(path)
    if not config.has_section("General"):
        config.add_section("General")
    config["General"].update({key: str(value) for key, value in values.items()})
    with path.open("w") as stream:
        config.write(stream, space_around_delimiters=False)


def configure_options(game):
    values = {
        "overrideWidth": "1280", "overrideHeight": "720", "fullscreen": "false",
        "enableVsync": "false", "renderDistance": "12", "maxFps": "0",
        "pauseOnLostFocus": "false", "snooperEnabled": "false",
    }
    for category in ("master", "music", "record", "weather", "block", "hostile",
                     "neutral", "player", "ambient", "voice"):
        values["soundCategory_" + category] = "0.0"
    path = game / "options.txt"
    publisher = game / ".benchmark-publisher-options.txt"
    require(not publisher.is_symlink(), "linked publisher options baseline")
    existing = publisher.read_text().splitlines() if publisher.exists() else []
    kept = [line for line in existing if line.partition(":")[0] not in values]
    path.write_text("\n".join(kept + [key + ":" + value for key, value in values.items()]) + "\n")
    return values


def mutable_cache_metadata(key):
    return (key.startswith(("prism/meta/", "prism/assets/indexes/")) or
            key.startswith("prism/libraries/") and Path(key).suffix.lower() in
            {".json", ".xml", ".sha1", ".md5", ".lastupdated"})


def pin_downloads(baseline, prism, pins):
    path = baseline / "inputs.toml"
    previous = tomllib.loads(path.read_text()).get("sha256", {}) if path.exists() else {}
    immutable, metadata, urls = dict(pins), {}, {}
    for name in ("libraries", "assets", "meta"):
        for item in sorted((prism / name).rglob("*")):
            require(not item.is_symlink(), "linked private cache input")
            if not item.is_file():
                continue
            key = "prism/" + str(item.relative_to(prism))
            destination = metadata if mutable_cache_metadata(key) else immutable
            destination[key] = digest(item)
            if destination is metadata:
                observed = re.findall(r'https?://[^\s"<>]+', item.read_text(errors="replace"))
                urls[key] = sorted(set(observed))
    for key, value in previous.items():
        if not mutable_cache_metadata(key):
            require(key in immutable and immutable[key] == value,
                    "previously pinned artifact changed or vanished: " + key)
    content = "schema_version = 2\n"
    for table, values in (("sha256", immutable), ("metadata_sha256", metadata),
                          ("metadata_observed_urls", urls)):
        content += "\n[" + table + "]\n"
        content += "".join(json.dumps(key) + " = " + json.dumps(value) + "\n"
                           for key, value in sorted(values.items()))
    temporary = path.with_suffix(".toml.tmp")
    temporary.write_text(content)
    temporary.replace(path)


def configure_instance(instance, java, agent, output):
    args = PACK_ARGS + ["-javaagent:" + str(agent) + "=" + str(output),
                        "-Xloggc:" + str(output / "gc.log"), "-XX:+PrintGCDetails",
                        "-XX:+PrintGCDateStamps", "-XX:+PrintGCTimeStamps"]
    update_ini(instance / "instance.cfg", {
        "name": "SevTech Ages 3.2.3 — original Forge", "InstanceType": "OneSix",
        "OverrideJavaLocation": "true", "JavaPath": java,
        "OverrideJavaArgs": "true", "JvmArgs": shlex.join(args),
        "OverrideMemory": "true", "MinMemAlloc": 4096, "MaxMemAlloc": 8192,
        "OverrideWindow": "true", "MinecraftWinWidth": 1280,
        "MinecraftWinHeight": 720, "LaunchMaximized": "false",
        "OverrideJava": "true", "JavaVersion": "1.8.0_504",
    })
    write_json(instance / "mmc-pack.json", {"formatVersion": 1, "components": COMPONENTS})
    return args


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--agent", type=Path, required=True, help="Java 8 observer agent JAR")
    parser.add_argument("--output", type=Path, required=True, help="observer and GC output directory")
    parser.add_argument("--cache-root", type=Path, help="optional existing Prism cache root")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[3]
    fixture = root / "tests/fixtures/local" / FIXTURE
    baseline = fixture / "baseline"
    prism = baseline / "prism"
    agent, output = args.agent.resolve(), args.output.resolve()
    require(agent.is_file(), "observer agent missing: " + str(agent))
    require(not any(c in str(agent) + str(output) for c in "\n\r="), "invalid agent/output path")
    archive, artifacts, pins = verify_inputs(root, fixture)
    java_home = subprocess.run(["mise", "where", JAVA], cwd=root, check=True,
                               text=True, capture_output=True).stdout.strip()
    java = Path(java_home) / "bin/java"
    require(java.is_file(), "Temurin 8 executable missing: " + str(java))
    cache_root = args.cache_root or fixture.resolve().parent / ("prism-" + "approot-headless")
    prism.mkdir(parents=True, exist_ok=True)
    configure_caches(prism, cache_root)
    pin_downloads(baseline, prism, pins)
    instance = prepare_instance(prism, archive, artifacts)
    output.mkdir(parents=True, exist_ok=True)
    jvm_args = configure_instance(instance, java, agent, output)
    options = configure_options(instance / ".minecraft")
    settings = {"fixture": FIXTURE, "instance": "original", "account": "BenchOffline",
                "prismRoot": str(prism), "javaDistribution": JAVA, "javaPath": str(java),
                "minMemoryMiB": 4096, "maxMemoryMiB": 8192, "jvmArgs": jvm_args,
                "recommendedArgsSource": ARGS_SOURCE, "components": COMPONENTS,
                "options": options, "artifactCount": len(artifacts),
                "agentSha256": digest(agent), "output": str(output)}
    write_json(baseline / "settings.json", settings)
    write_json(output / "settings.json", settings)
    print(json.dumps(settings, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, zipfile.BadZipFile, subprocess.CalledProcessError) as error:
        sys.exit("baseline preparation refused: " + str(error))
