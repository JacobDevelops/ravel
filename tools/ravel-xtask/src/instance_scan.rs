//! Proves that a launcher instance carries no competing mod loader.
//!
//! M3-W7's gate asks for an instance scan showing that the process Ravel ran in
//! had no Fabric, `NeoForge`, Forge, or Quilt present. That claim was previously
//! made by a person looking at a directory, which is not evidence anyone else can
//! recheck. This is the mechanism that makes it one.
//!
//! # Why nothing read here can be reported
//!
//! The scan runs against a real instance on a real machine — the same directory
//! that holds the player's worlds, the launcher's account state, and Ravel's own
//! wrapper command with its absolute paths — and its output becomes tracked
//! evidence. A scanner that leaks is worse than no scanner, because the leak
//! ships in the repository and the missing scan does not.
//!
//! So the report is closed rather than redacted. [`Finding`] and [`InstanceScan`]
//! hold `&'static str` and integers and nothing else: every string that leaves
//! this module is a marker constant written below, and the only file-derived
//! values are counts. Filenames, path segments, versions, and file contents are
//! read, compared against constants, and dropped. Redaction is therefore a
//! property of the types rather than a filter that has to be remembered, and
//! [`tests::a_username_shaped_path_segment_never_reaches_the_report`] is what
//! holds it there.

use std::fs;
use std::path::{Path, PathBuf};

/// A marker that proves a competing loader is present, and why it proves it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Finding {
    /// The loader the marker belongs to.
    pub loader: &'static str,
    /// The marker itself: a Prism component uid, a relative path, or a class name.
    pub marker: &'static str,
    /// How the marker was found, which is what says how strong it is.
    pub kind: &'static str,
}

/// What scanning one launcher instance found.
///
/// `conclusive` is separate from `loader_present` because they answer different
/// questions. An instance that could not be read has no loader marker either,
/// and reporting that as a clean scan would turn every unreadable directory into
/// a proof of absence. M3-W7 needs the absence proved, so a scan that could not
/// look fails closed and says why.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InstanceScan {
    /// Every marker found, in the order the markers are declared below.
    pub findings: Vec<Finding>,
    /// Whether the scan was able to look everywhere it claims to have looked.
    pub conclusive: bool,
    /// What stopped the scan from concluding, if anything did.
    pub obstacles: Vec<&'static str>,
    /// How many mod archives sit in the game directory's `mods` folder.
    pub mod_archives: usize,
}

impl InstanceScan {
    /// Whether any competing loader was proven present.
    #[must_use]
    pub fn loader_present(&self) -> bool {
        !self.findings.is_empty()
    }

    /// Whether the instance is proven free of every competing loader.
    ///
    /// An inconclusive scan is not clean. This is the single value M3-W7's
    /// `instance.withoutAnotherLoader` claim is allowed to come from.
    #[must_use]
    pub fn without_another_loader(&self) -> bool {
        self.conclusive && self.findings.is_empty() && self.mod_archives == 0
    }

    /// Renders the scan as the sanitized JSON object an evidence record embeds.
    ///
    /// Written by hand rather than serialized because the point of the type is
    /// that its fields are constants; a derive would invite a field that is not.
    /// Every string interpolated here is a constant declared in this module, so
    /// none of them can need escaping.
    #[must_use]
    pub fn to_json(&self) -> String {
        let findings = self
            .findings
            .iter()
            .map(|finding| {
                format!(
                    "\n      {{ \"loader\": \"{}\", \"marker\": \"{}\", \"kind\": \"{}\" }}",
                    finding.loader, finding.marker, finding.kind
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let obstacles = self
            .obstacles
            .iter()
            .map(|obstacle| format!("\n      \"{obstacle}\""))
            .collect::<Vec<_>>()
            .join(",");
        let close = |items: &str| if items.is_empty() { "]" } else { "\n    ]" };
        let findings_close = close(&findings);
        let obstacles_close = close(&obstacles);
        let loader_present = self.loader_present();
        let without_another_loader = self.without_another_loader();
        let conclusive = self.conclusive;
        let archives = self.mod_archives;
        format!(
            "{{\n  \"scan\": \"ravel-instance-scan-v1\",\n  \
             \"loaderPresent\": {loader_present},\n  \"conclusive\": {conclusive},\n  \
             \"withoutAnotherLoader\": {without_another_loader},\n  \
             \"modArchives\": {archives},\n  \
             \"findings\": [{findings}{findings_close},\n  \
             \"obstacles\": [{obstacles}{obstacles_close}\n}}"
        )
    }
}

/// A marker that proves one loader is installed, and the reason it proves it.
struct Marker {
    loader: &'static str,
    token: &'static str,
    kind: &'static str,
}

/// Loader components as the launcher itself names them.
///
/// This is the strongest marker there is, and the only one that proves intent
/// rather than residue: a Prism instance's `mmc-pack.json` and `patches/*.json`
/// are the launcher's own declaration of what it will put on the classpath and
/// which main class it will start. A uid here means the loader is not merely on
/// disk, it is what the next launch runs. Every uid is the canonical one each
/// project publishes to the launcher metadata service, so a match is exact.
const COMPONENT_MARKERS: &[Marker] = &[
    Marker {
        loader: "fabric",
        token: "net.fabricmc.fabric-loader",
        kind: "launcher-component",
    },
    Marker {
        loader: "fabric",
        token: "net.fabricmc.intermediary",
        kind: "launcher-component",
    },
    Marker {
        loader: "quilt",
        token: "org.quiltmc.quilt-loader",
        kind: "launcher-component",
    },
    Marker {
        loader: "quilt",
        token: "org.quiltmc.hashed",
        kind: "launcher-component",
    },
    Marker {
        loader: "forge",
        token: "net.minecraftforge",
        kind: "launcher-component",
    },
    Marker {
        loader: "neoforge",
        token: "net.neoforged",
        kind: "launcher-component",
    },
];

/// Launch entry points that only a loader installs.
///
/// A loader does not start the game's own main class; it starts its own and
/// hands control on once it has rewritten the classpath. Finding one of these in
/// the launcher's metadata proves the wrapper is in the launch path, which is a
/// separate fact from the component being installed — an instance can carry a
/// disabled component, but it cannot carry a main class it does not launch.
const LAUNCH_MARKERS: &[Marker] = &[
    Marker {
        loader: "fabric",
        token: "net.fabricmc.loader.impl.launch.knot.KnotClient",
        kind: "launch-main-class",
    },
    Marker {
        loader: "fabric",
        token: "net.fabricmc.loader.impl.launch.knot.KnotServer",
        kind: "launch-main-class",
    },
    Marker {
        loader: "quilt",
        token: "org.quiltmc.loader.impl.launch.knot.KnotClient",
        kind: "launch-main-class",
    },
    Marker {
        loader: "forge-family",
        token: "cpw.mods.bootstraplauncher.BootstrapLauncher",
        kind: "launch-main-class",
    },
    Marker {
        loader: "forge",
        token: "net.minecraftforge.bootstrap.ForgeBootstrap",
        kind: "launch-main-class",
    },
    Marker {
        loader: "neoforge",
        token: "net.neoforged.bootstrap.Bootstrap",
        kind: "launch-main-class",
    },
];

/// Files the launcher writes to record a component override, relative to the
/// instance root.
///
/// Prism names a patch file after the component it patches, so the filename
/// alone carries the claim and nothing inside it has to be read.
const PATCH_MARKERS: &[Marker] = &[
    Marker {
        loader: "fabric",
        token: "patches/net.fabricmc.fabric-loader.json",
        kind: "component-patch",
    },
    Marker {
        loader: "quilt",
        token: "patches/org.quiltmc.quilt-loader.json",
        kind: "component-patch",
    },
    Marker {
        loader: "forge",
        token: "patches/net.minecraftforge.json",
        kind: "component-patch",
    },
    Marker {
        loader: "neoforge",
        token: "patches/net.neoforged.json",
        kind: "component-patch",
    },
];

/// What a loader leaves in the game directory, relative to the game root.
///
/// These are residue rather than declaration, and that is exactly what makes
/// them worth checking: they are written by the loader at runtime, so they
/// survive a component being removed from the launcher and prove the instance
/// has run a loader even when the launcher no longer says so. The vanilla client
/// creates none of them — it has no notion of a mod, a mixin, or a loader
/// dependency — so each one's presence has only one explanation.
const GAME_DIRECTORY_MARKERS: &[Marker] = &[
    Marker {
        loader: "fabric",
        token: ".fabric",
        kind: "loader-runtime-directory",
    },
    Marker {
        loader: "fabric",
        token: "config/fabric_loader_dependencies.json",
        kind: "loader-config",
    },
    Marker {
        loader: "quilt",
        token: ".quilt",
        kind: "loader-runtime-directory",
    },
    Marker {
        loader: "quilt",
        token: "config/quilt_loader_dependencies.json",
        kind: "loader-config",
    },
    Marker {
        loader: "forge-family",
        token: "config/fml.toml",
        kind: "loader-config",
    },
    Marker {
        loader: "forge-family",
        token: "defaultconfigs",
        kind: "loader-config",
    },
    Marker {
        loader: "forge",
        token: "config/forge-client.toml",
        kind: "loader-config",
    },
    Marker {
        loader: "neoforge",
        token: "config/neoforge-client.toml",
        kind: "loader-config",
    },
    // Every one of the four is built on Mixin, and Mixin writes its transformed
    // classes here. Nothing else in a Minecraft install does.
    Marker {
        loader: "mixin-based-loader",
        token: ".mixin.out",
        kind: "loader-runtime-directory",
    },
];

/// Library trees a loader unpacks, relative to a directory that holds libraries.
///
/// A loader jar is proof the instance can start one even with no component and
/// no residue, which is the case a freshly imported modpack presents.
const LIBRARY_MARKERS: &[Marker] = &[
    Marker {
        loader: "fabric",
        token: "libraries/net/fabricmc",
        kind: "loader-library",
    },
    Marker {
        loader: "quilt",
        token: "libraries/org/quiltmc",
        kind: "loader-library",
    },
    Marker {
        loader: "forge",
        token: "libraries/net/minecraftforge",
        kind: "loader-library",
    },
    Marker {
        loader: "neoforge",
        token: "libraries/net/neoforged",
        kind: "loader-library",
    },
];

/// The launcher metadata that is searched for component and launch markers.
const LAUNCHER_METADATA: &[&str] = &["mmc-pack.json", "instance.cfg"];

/// Scans one launcher instance for every competing loader Ravel must exclude.
///
/// `root` is the instance directory, and it is used to open files and never to
/// describe them: see the module comment for why the report cannot carry it.
#[must_use]
pub fn scan_instance(root: &Path) -> InstanceScan {
    let mut scan = InstanceScan {
        conclusive: true,
        ..InstanceScan::default()
    };

    if !root.is_dir() {
        scan.conclusive = false;
        scan.obstacles
            .push("the instance directory does not exist or is not a directory");
        return scan;
    }

    let metadata = launcher_metadata(root);
    if metadata.is_empty() {
        // A directory with no launcher metadata may be an instance laid out some
        // other way, and this scan cannot tell. Saying so is the whole point:
        // the alternative is an empty finding list that reads like a clean scan.
        scan.conclusive = false;
        scan.obstacles
            .push("no launcher metadata was found, so the launch configuration was not scanned");
    }
    let declarations = metadata
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .collect::<Vec<_>>();
    if declarations.len() != metadata.len() {
        scan.conclusive = false;
        scan.obstacles
            .push("some launcher metadata could not be read, so its declarations were not scanned");
    }
    for marker in COMPONENT_MARKERS.iter().chain(LAUNCH_MARKERS) {
        if declarations
            .iter()
            .any(|declaration| declaration.contains(marker.token))
        {
            scan.findings.push(finding(marker));
        }
    }

    for marker in PATCH_MARKERS {
        if root.join(marker.token).exists() {
            scan.findings.push(finding(marker));
        }
    }

    let Some(game) = game_directory(root) else {
        scan.conclusive = false;
        scan.obstacles
            .push("no game directory was found, so mods and loader residue were not scanned");
        return scan;
    };

    for marker in GAME_DIRECTORY_MARKERS {
        if game.join(marker.token).exists() {
            scan.findings.push(finding(marker));
        }
    }
    for marker in LIBRARY_MARKERS {
        if root.join(marker.token).exists() || game.join(marker.token).exists() {
            scan.findings.push(finding(marker));
        }
    }

    match mod_archives(&game.join("mods")) {
        // A mod archive is counted rather than named, and it is counted at all
        // because the vanilla client never reads this directory. Whatever put an
        // archive there expected something to load it, and Ravel loads Rust
        // packages rather than jars, so the count is a claim about a loader even
        // when no loader marker was found.
        Ok(count) => scan.mod_archives = count,
        Err(obstacle) => {
            scan.conclusive = false;
            scan.obstacles.push(obstacle);
        }
    }

    scan
}

fn finding(marker: &Marker) -> Finding {
    Finding {
        loader: marker.loader,
        marker: marker.token,
        kind: marker.kind,
    }
}

/// The launcher metadata files that exist under an instance root.
fn launcher_metadata(root: &Path) -> Vec<PathBuf> {
    let mut found = LAUNCHER_METADATA
        .iter()
        .map(|name| root.join(name))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    let mut patches = fs::read_dir(root.join("patches"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    // Directory order is not stable across filesystems, and a report whose
    // finding order depends on that is not comparable between two runs.
    patches.sort();
    found.append(&mut patches);
    found
}

/// The game directory inside an instance.
///
/// Prism writes `.minecraft` on Linux and `minecraft` elsewhere, and a plain
/// vanilla install is its own game directory. All three are tried so the scan
/// does not silently skip the half of the markers that live under it.
fn game_directory(root: &Path) -> Option<PathBuf> {
    [".minecraft", "minecraft"]
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_dir())
        .or_else(|| root.join("options.txt").is_file().then(|| root.to_owned()))
        .or_else(|| root.join("mods").is_dir().then(|| root.to_owned()))
}

/// Counts the archives in a `mods` directory without naming any of them.
///
/// A disabled archive counts. Every launcher's disable feature is a rename, so
/// an instance one rename away from loading a pack is not an instance without
/// one.
fn mod_archives(mods: &Path) -> Result<usize, &'static str> {
    if !mods.exists() {
        return Ok(0);
    }
    let entries = fs::read_dir(mods)
        .map_err(|_| "the mods directory could not be read, so its archives were not counted")?;
    let mut count = 0;
    for entry in entries {
        let entry =
            entry.map_err(|_| "a mods directory entry could not be read, so it was not counted")?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let enabled = name.strip_suffix(".disabled").unwrap_or(&name);
        if Path::new(enabled)
            .extension()
            .is_some_and(|extension| extension == "jar")
        {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{InstanceScan, scan_instance};

    /// The reason this module exists in the shape it does.
    ///
    /// The instance path on the machine that will run this holds the operator's
    /// username, and the instance itself holds the launcher's account state and
    /// Ravel's wrapper command. This feeds the scanner a root whose every
    /// segment is username-shaped, plants a loader so the report is as full as
    /// it can be, and asserts that none of those segments survives into it.
    /// Both the rendered JSON and the structured report are checked, because a
    /// leak that only reached one of them is still a leak.
    #[test]
    fn a_username_shaped_path_segment_never_reaches_the_report() {
        let secret = "jacob-sanderson";
        let root = temporary_root("redaction");
        let instance = root
            .join(secret)
            .join("instances")
            .join(format!("{secret}-atm11"));
        let game = instance.join(".minecraft");
        write(&instance.join("mmc-pack.json"), &fabric_pack(secret));
        write(
            &instance.join("instance.cfg"),
            &format!("name={secret}\nJvmArgs=-Dravel.home=/home/{secret}/ravel\n"),
        );
        write(
            &instance.join("patches/net.fabricmc.fabric-loader.json"),
            &format!("{{ \"name\": \"{secret}\" }}"),
        );
        write(&game.join(format!("mods/{secret}-pack.jar")), "archive");
        write(&game.join(format!("config/{secret}.toml")), "setting = 1");
        write(&game.join("config/fabric_loader_dependencies.json"), "{}");
        write(
            &game.join(format!("saves/{secret}-world/level.dat")),
            "world",
        );

        let scan = scan_instance(&instance);

        assert!(scan.loader_present(), "the planted loader should be found");
        assert!(!scan.without_another_loader());
        assert_eq!(scan.mod_archives, 1);
        let rendered = scan.to_json();
        assert!(
            !rendered.contains(secret),
            "the report must not carry a path segment from the instance"
        );
        for finding in &scan.findings {
            assert!(!finding.loader.contains(secret));
            assert!(!finding.marker.contains(secret));
            assert!(!finding.kind.contains(secret));
        }
        for obstacle in &scan.obstacles {
            assert!(!obstacle.contains(secret));
        }
        // The absolute root is what a careless implementation would echo, and
        // its own segments are the ones a reader would recognize.
        for segment in &instance {
            let segment = segment.to_string_lossy();
            assert!(
                segment == "/" || !rendered.contains(segment.as_ref()),
                "the report must not carry the instance path"
            );
        }
        remove(&root);
    }

    #[test]
    fn a_stock_instance_is_proven_free_of_every_loader() {
        let root = temporary_root("stock");
        let instance = root.join("vanilla");
        write(
            &instance.join("mmc-pack.json"),
            r#"{"components":[{"uid":"net.minecraft","version":"26.1.2"},
                {"uid":"org.lwjgl3","version":"3.4.1"}],"formatVersion":1}"#,
        );
        write(&instance.join(".minecraft/options.txt"), "fov:0.0\n");

        let scan = scan_instance(&instance);

        assert_eq!(scan.findings, []);
        assert_eq!(scan.obstacles, [] as [&str; 0]);
        assert!(scan.conclusive);
        assert!(scan.without_another_loader());
        remove(&root);
    }

    #[test]
    fn every_loader_is_found_by_the_marker_that_proves_it() {
        for (name, plant) in [
            ("fabric", "mmc-pack:net.fabricmc.fabric-loader"),
            ("quilt", "mmc-pack:org.quiltmc.quilt-loader"),
            ("forge", "mmc-pack:net.minecraftforge"),
            ("neoforge", "mmc-pack:net.neoforged"),
        ] {
            let root = temporary_root(name);
            let instance = root.join("instance");
            let uid = plant.trim_start_matches("mmc-pack:");
            write(
                &instance.join("mmc-pack.json"),
                &format!(r#"{{"components":[{{"uid":"{uid}","version":"1"}}]}}"#),
            );
            write(&instance.join(".minecraft/options.txt"), "fov:0.0\n");

            let scan = scan_instance(&instance);

            assert!(
                scan.findings.iter().any(|finding| finding.marker == uid),
                "{name} should be found by its launcher component"
            );
            assert!(!scan.without_another_loader());
            remove(&root);
        }
    }

    /// An install with no launcher wrapper is still an instance to scan.
    ///
    /// Prism nests the game under `.minecraft`, but a plain install is its own
    /// game directory, and the residue markers are the only ones it can carry.
    /// Without this the fallback would be reached by nothing, and a marker that
    /// is never looked for is indistinguishable from one that found nothing.
    #[test]
    fn a_game_directory_given_directly_is_scanned_for_residue() {
        let root = temporary_root("plain-install");
        let game = root.join("game");
        write(&game.join("options.txt"), "fov:0.0\n");
        write(&game.join(".fabric/remappedJars/marker"), "residue");

        let scan = scan_instance(&game);

        assert!(
            scan.findings
                .iter()
                .any(|finding| finding.marker == ".fabric"),
            "loader residue in a plain install should be found"
        );
        // The launcher metadata is genuinely absent rather than unreadable, so
        // the scan says what it could not check instead of passing the instance.
        assert!(!scan.conclusive);
        assert!(!scan.without_another_loader());

        // A directory that holds only `mods` is a game directory too, and it is
        // the one shape where the archives are the whole of the evidence.
        let bare = root.join("bare-mods");
        write(&bare.join("mods/pack.jar"), "archive");
        assert_eq!(scan_instance(&bare).mod_archives, 1);
        remove(&root);
    }

    /// An unreadable instance is not a clean instance.
    ///
    /// This is the failure the whole `conclusive` field exists for: a scan that
    /// found nothing because it could not look reads exactly like a scan that
    /// found nothing because there was nothing there.
    #[test]
    fn a_scan_that_could_not_look_does_not_prove_absence() {
        let root = temporary_root("inconclusive");
        let absent = scan_instance(&root.join("not-an-instance"));
        assert!(!absent.conclusive);
        assert!(!absent.without_another_loader());
        assert_eq!(absent.findings, []);

        let bare = root.join("bare");
        fs::create_dir_all(&bare).expect("instance directory should be creatable");
        let empty = scan_instance(&bare);
        assert!(!empty.conclusive);
        assert!(!empty.without_another_loader());
        remove(&root);
    }

    /// A jar in `mods` is a loader claim even when no loader marker was found.
    #[test]
    fn a_mod_archive_alone_refuses_the_absence_claim() {
        let root = temporary_root("archives");
        let instance = root.join("instance");
        write(&instance.join("mmc-pack.json"), r#"{"components":[]}"#);
        write(&instance.join(".minecraft/options.txt"), "fov:0.0\n");
        write(&instance.join(".minecraft/mods/one.jar"), "archive");
        write(
            &instance.join(".minecraft/mods/two.JAR.disabled"),
            "archive",
        );
        write(
            &instance.join(".minecraft/mods/notes.txt"),
            "not an archive",
        );

        let scan = scan_instance(&instance);

        assert_eq!(scan.mod_archives, 2);
        assert_eq!(scan.findings, []);
        assert!(scan.conclusive);
        assert!(!scan.without_another_loader());
        remove(&root);
    }

    #[test]
    fn the_rendered_report_is_valid_json_in_both_of_its_shapes() {
        let clean = InstanceScan {
            conclusive: true,
            ..InstanceScan::default()
        };
        assert!(clean.to_json().contains("\"findings\": [],"));
        assert!(clean.to_json().contains("\"withoutAnotherLoader\": true"));

        let root = temporary_root("render");
        let instance = root.join("instance");
        write(
            &instance.join("mmc-pack.json"),
            r#"{"components":[{"uid":"net.neoforged","version":"26.1.2.78"}]}"#,
        );
        write(&instance.join(".minecraft/options.txt"), "fov:0.0\n");
        let rendered = scan_instance(&instance).to_json();
        assert!(rendered.contains("\"loader\": \"neoforge\""));
        assert!(rendered.contains("\"withoutAnotherLoader\": false"));
        remove(&root);
    }

    fn fabric_pack(secret: &str) -> String {
        format!(
            r#"{{"components":[{{"uid":"net.minecraft","version":"26.1.2"}},
                {{"uid":"net.fabricmc.fabric-loader","version":"0.18.1"}}],
                "name":"{secret}"}}"#
        )
    }

    fn write(path: &Path, contents: &str) {
        let parent = path.parent().expect("a fixture file should have a parent");
        fs::create_dir_all(parent).expect("fixture directory should be creatable");
        fs::write(path, contents).expect("fixture file should be writable");
    }

    fn remove(root: &Path) {
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }

    fn temporary_root(label: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("ravel-xtask-scan-{label}-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale temporary root should be removable");
        }
        fs::create_dir_all(&root).expect("temporary root should be creatable");
        root
    }
}
