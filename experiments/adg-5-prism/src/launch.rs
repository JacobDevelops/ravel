//! Reads an operator's instance log and says what it proves about a launch.
//!
//! The launch itself is the operator's: it needs their signed-in session, and
//! nothing here authenticates or starts anything. What this module removes is the
//! step after it — hand-authoring a schema-conformant result — which is where a
//! recorded token or a wrong hash would come from.
//!
//! A launch log can contain the account's session arguments — whether it does
//! depends on how the launcher prints its command, so redaction runs regardless.
//! They are removed before anything is hashed, and the log's text never reaches a
//! result: only the hash of its redacted form does, matching how M0 recorded its
//! launches.

use std::fmt::Write as _;

/// Arguments whose value is the account's, and never evidence.
const SECRET_ARGUMENTS: &[&str] = &[
    "--accessToken",
    "--session",
    "--uuid",
    "--username",
    "--xuid",
    "--clientId",
];

/// What the operator's log shows.
#[derive(Debug, PartialEq, Eq)]
pub struct Launch {
    /// Whether the agent argument reached the game's command line.
    pub agent: bool,
    /// Whether the library path reached the game's command line.
    pub library_path: bool,
    /// Another Java mod loader named in the log, if one is.
    pub other_loader: Option<String>,
    /// Digest of the redacted log, which is what a result may record.
    pub redacted_sha256: String,
}

impl Launch {
    /// Whether this log proves a stock launch carrying Ravel's arguments.
    #[must_use]
    pub fn proves_stock_launch(&self) -> bool {
        self.agent && self.library_path && self.other_loader.is_none()
    }

    /// Why it does not, in the words a result should record.
    #[must_use]
    pub fn objection(&self) -> Option<String> {
        if let Some(loader) = &self.other_loader {
            return Some(format!(
                "the log names {loader}, so this launch does not show the official game running \
                 without another mod loader"
            ));
        }
        match (self.agent, self.library_path) {
            (true, true) => None,
            (false, true) => Some("the launch command carries no -javaagent".to_owned()),
            (true, false) => Some("the launch command carries no -Djava.library.path".to_owned()),
            (false, false) => Some(
                "the launch command carries neither of Ravel's two arguments, so this is a \
                 launch of the unmodified game"
                    .to_owned(),
            ),
        }
    }
}

/// Names that mean a Java mod loader was present.
const OTHER_LOADERS: &[&str] = &[
    "neoforge",
    "fabricmc",
    "fabric-loader",
    "modlauncher",
    "quilt",
];

/// Reads one instance log.
///
/// The log is consumed as text and dropped; nothing is returned that could carry
/// the account's arguments onward.
#[must_use]
pub fn read(log: &str) -> Launch {
    let redacted = redact(log);
    let lower = redacted.to_ascii_lowercase();
    Launch {
        agent: redacted.contains("-javaagent:"),
        library_path: redacted.contains("-Djava.library.path="),
        other_loader: OTHER_LOADERS
            .iter()
            .find(|loader| lower.contains(**loader))
            .map(|loader| (*loader).to_owned()),
        redacted_sha256: ravel_spike_harness::sha256_bytes(redacted.as_bytes()),
    }
}

/// Replaces the account's argument values, leaving the arguments themselves.
///
/// The argument names are evidence — they show the launcher built a normal
/// command — while the values never are.
#[must_use]
pub fn redact(log: &str) -> String {
    let mut redacted = String::with_capacity(log.len());
    for line in log.lines() {
        let mut redact_next = false;
        for (index, word) in line.split(' ').enumerate() {
            if index > 0 {
                redacted.push(' ');
            }
            // A launcher prints its command as a list, so an argument arrives as
            // `--accessToken,` rather than `--accessToken`. Matching the bare word
            // is the difference between redacting a token and recording one.
            let bare = word.trim_matches([',', '[', ']']);
            if redact_next {
                redact_next = false;
                let _ = write!(redacted, "[redacted]{}", punctuation(word));
                continue;
            }
            if SECRET_ARGUMENTS.contains(&bare) {
                redact_next = true;
                redacted.push_str(word);
                continue;
            }
            match bare.split_once('=') {
                Some((name, _)) if SECRET_ARGUMENTS.contains(&name) => {
                    let _ = write!(redacted, "{name}=[redacted]{}", punctuation(word));
                }
                _ => redacted.push_str(word),
            }
        }
        redacted.push('\n');
    }
    redacted
}

/// Trailing list punctuation, kept so a redacted log still reads like a log.
fn punctuation(word: &str) -> &str {
    let kept = word.trim_end_matches([',', ']']);
    &word[kept.len()..]
}

#[cfg(test)]
mod tests {
    use super::{read, redact};

    /// A launch line shaped like the launcher's own, with a fake session.
    fn log(extra: &str) -> String {
        format!(
            "[Launcher] Java path is /runtime/bin/java\n\
             [Launcher] Main class: net.minecraft.client.main.Main\n\
             [Launcher] Java Arguments:\n\
             [/runtime/bin/java, -Xms512m, {extra}-cp, /instance/client.jar, \
             net.minecraft.client.main.Main, --username, Notch, --accessToken, \
             eyJhbGciOiJIUzI1NiJ9.notarealtoken, --uuid=00000000000000000000000000000000]\n\
             [Launcher] Minecraft process ID: 1234\n"
        )
    }

    #[test]
    fn a_launch_carrying_both_arguments_proves_a_stock_launch() {
        let observed = read(&log(
            "-javaagent:/instance/ravel-agent.jar=x, -Djava.library.path=/instance/natives, ",
        ));

        assert!(observed.agent);
        assert!(observed.library_path);
        assert_eq!(observed.other_loader, None);
        assert!(observed.proves_stock_launch());
        assert_eq!(observed.objection(), None);
    }

    #[test]
    fn an_unmodified_launch_proves_nothing_about_ravel() {
        let observed = read(&log(""));

        assert!(!observed.proves_stock_launch());
        assert!(
            observed
                .objection()
                .expect("an unmodified launch must be objected to")
                .contains("neither of Ravel's two arguments")
        );
    }

    #[test]
    fn a_launch_with_another_loader_is_refused_by_name() {
        let observed = read(&log(
            "-javaagent:/instance/ravel-agent.jar=x, -Djava.library.path=/n, \
             -Dneoforge.version=26.1.2.78, ",
        ));

        assert_eq!(observed.other_loader.as_deref(), Some("neoforge"));
        assert!(!observed.proves_stock_launch());
        assert!(
            observed
                .objection()
                .expect("another loader must be objected to")
                .contains("neoforge")
        );
    }

    #[test]
    fn the_accounts_arguments_never_survive_redaction() {
        let redacted = redact(&log(""));

        assert!(!redacted.contains("notarealtoken"), "a token survived");
        assert!(!redacted.contains("Notch"), "a username survived");
        assert!(
            !redacted.contains("00000000000000000000000000000000"),
            "a uuid survived"
        );
        // The argument names stay: they are what shows a normal command was built.
        // The launcher's list punctuation survives; only the values do not.
        assert!(
            redacted.contains("--accessToken, [redacted],"),
            "{redacted}"
        );
        assert!(redacted.contains("--username, [redacted],"), "{redacted}");
        assert!(redacted.contains("--uuid=[redacted]]"), "{redacted}");
        assert!(redacted.contains("net.minecraft.client.main.Main"));
    }

    #[test]
    fn the_same_log_always_hashes_the_same_way() {
        let first = read(&log("-javaagent:/a.jar=x, -Djava.library.path=/n, "));
        let second = read(&log("-javaagent:/a.jar=x, -Djava.library.path=/n, "));

        assert_eq!(first.redacted_sha256, second.redacted_sha256);
        assert_ne!(first.redacted_sha256, read(&log("")).redacted_sha256);
    }
}
