use ravel_core::{DiagnosticMessage, RedactionPolicy};

const REDACTED: &str = "[redacted]";

/// Case-insensitive substrings identifying credential-bearing labels.
pub const CREDENTIAL_WORDS: [&str; 5] =
    ["token", "password", "secret", "credential", "authorization"];

/// Case-sensitive prefixes identifying credential values without a label.
pub const CREDENTIAL_PREFIXES: [&str; 6] = ["ghp_", "gho_", "ghs_", "xoxb-", "xoxp-", "sk-"];

// Short dotted names are not tokens.
const TOKEN_SEGMENT: usize = 8;

const OPENING: [char; 5] = ['`', '\'', '"', '(', '['];
const CLOSING: [char; 9] = ['`', '\'', '"', ')', ']', '.', ',', ';', '!'];

/// Reduces absolute paths to filenames and replaces credentials with `[redacted]`.
///
/// Ambiguous credential labels over-redact; URLs and package-relative paths survive.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Redactor;

impl RedactionPolicy for Redactor {
    fn apply(&self, text: &str) -> String {
        let mut redacted = String::with_capacity(text.len());
        let mut rest = text;
        let mut value_follows = false;

        while !rest.is_empty() {
            let gap = rest
                .find(|character: char| !character.is_whitespace())
                .unwrap_or(rest.len());
            redacted.push_str(&rest[..gap]);
            rest = &rest[gap..];
            if rest.is_empty() {
                break;
            }

            let (word, tail) = split_word(rest);
            let (opening, core, closing) = undecorate(word);
            let (core, closing, tail) = follow_a_path_across_spaces(core, closing, tail);
            redacted.push_str(opening);
            redacted.push_str(&redact_word(core, value_follows));
            redacted.push_str(closing);
            value_follows = names_a_credential(core);
            rest = tail;
        }

        redacted
    }
}

/// Redacts `text` and returns it as a message that may be shown to a player.
#[must_use]
pub fn redact(text: &str) -> DiagnosticMessage {
    DiagnosticMessage::redacted(&Redactor, text)
}

fn split_word(rest: &str) -> (&str, &str) {
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    rest.split_at(end)
}

fn follow_a_path_across_spaces<'text>(
    core: &'text str,
    closing: &'text str,
    tail: &'text str,
) -> (&'text str, &'text str, &'text str) {
    if !closing.is_empty() || !is_absolute(core) {
        return (core, closing, tail);
    }

    let (mut core, mut closing, mut tail) = (core, closing, tail);
    loop {
        let Some(gap) = tail.find(|character: char| !character.is_whitespace()) else {
            return (core, closing, tail);
        };
        if tail[..gap].contains('\n') {
            return (core, closing, tail);
        }
        let (word, remainder) = split_word(&tail[gap..]);
        let (opening, next, next_closing) = undecorate(word);
        if !opening.is_empty() || !continues_a_path(next) {
            return (core, closing, tail);
        }
        (core, closing, tail) = (last_component(next), next_closing, remainder);
        if !closing.is_empty() {
            return (core, closing, tail);
        }
    }
}

fn continues_a_path(core: &str) -> bool {
    core.contains(separator) && !is_absolute(core) && !core.contains("://")
}

fn undecorate(word: &str) -> (&str, &str, &str) {
    let unopened = word.trim_start_matches(OPENING);
    let opening = &word[..word.len() - unopened.len()];
    let core = unopened.trim_end_matches(CLOSING);
    let closing = &word[opening.len() + core.len()..];
    (opening, core, closing)
}

// Value rules inspect filenames; credential labels use the original word.
fn redact_word(core: &str, value_follows: bool) -> String {
    if core.is_empty() {
        return String::new();
    }
    // Preserve credential labels, but redact the following value.
    if names_a_credential(core) {
        return core.to_owned();
    }
    let core = file_name_of(core).unwrap_or(core);
    if let Some((name, value)) = core.split_once('=')
        && !value.is_empty()
        && contains_a_credential_word(name)
    {
        return format!("{name}={REDACTED}");
    }
    if value_follows || starts_with_a_credential_prefix(core) || has_the_shape_of_a_token(core) {
        return REDACTED.to_owned();
    }
    core.to_owned()
}

// Exact labels avoid consuming text after filenames or `name=value` pairs.
fn names_a_credential(core: &str) -> bool {
    let name = core.trim_end_matches(':').to_ascii_lowercase();
    name == "bearer" || CREDENTIAL_WORDS.contains(&name.as_str())
}

fn contains_a_credential_word(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    CREDENTIAL_WORDS.iter().any(|word| lowered.contains(word))
}

fn starts_with_a_credential_prefix(core: &str) -> bool {
    CREDENTIAL_PREFIXES
        .iter()
        .any(|prefix| core.starts_with(prefix))
}

// Require three long base64url segments, excluding ordinary versions and names.
fn has_the_shape_of_a_token(core: &str) -> bool {
    let mut segments = core.split('.');
    let shape = [segments.next(), segments.next(), segments.next()];
    segments.next().is_none()
        && shape.iter().all(|segment| {
            segment.is_some_and(|segment| {
                segment.len() >= TOKEN_SEGMENT
                    && segment
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
            })
        })
}

// Relative paths identify package content rather than the player's filesystem.
fn file_name_of(core: &str) -> Option<&str> {
    if !is_absolute(core) {
        return None;
    }
    Some(last_component(core))
}

fn last_component(core: &str) -> &str {
    core.rsplit(separator)
        .find(|component| !component.is_empty())
        .unwrap_or(REDACTED)
}

fn is_absolute(core: &str) -> bool {
    if core.starts_with('/') || core.starts_with("~/") || core.starts_with("\\\\") {
        return true;
    }
    let drive = core.as_bytes();
    drive.len() >= 3
        && drive[0].is_ascii_alphabetic()
        && drive[1] == b':'
        && (drive[2] == b'/' || drive[2] == b'\\')
}

fn separator(character: char) -> bool {
    matches!(character, '/' | '\\')
}

#[cfg(test)]
mod tests {
    use ravel_core::{DiagnosticMessage, Redaction, RedactionPolicy};

    use super::{Redactor, redact};

    fn applied(text: &str) -> String {
        let message = redact(text);
        assert_eq!(message.redaction(), Redaction::Applied);
        assert!(message.is_publishable());
        message.text().to_owned()
    }

    #[test]
    fn an_absolute_path_is_reduced_to_the_file_it_names() {
        for (raw, expected) in [
            ("/home/player/.minecraft/mods/level.dat", "level.dat"),
            ("/home/player/", "player"),
            ("~/games/instance/config.toml", "config.toml"),
            ("C:\\Users\\player\\saves\\level.dat", "level.dat"),
            ("c:/Users/player/saves", "saves"),
            ("\\\\server\\share\\mod.bin", "mod.bin"),
            ("/", "[redacted]"),
        ] {
            assert_eq!(applied(raw), expected, "{raw}");
        }
    }

    #[test]
    fn a_path_inside_a_sentence_is_reduced_without_disturbing_the_sentence() {
        assert_eq!(
            applied("`example_mod` refused: could not open /home/player/.minecraft/saves/world"),
            "`example_mod` refused: could not open world"
        );
    }

    #[test]
    fn a_path_is_followed_across_the_spaces_inside_a_directory_name() {
        for (raw, expected) in [
            (
                "could not open /home/player/My Games/saves/level.dat",
                "could not open level.dat",
            ),
            (
                "`/home/player/My Games/mods`, which is not readable",
                "`mods`, which is not readable",
            ),
            ("/home/player/My Games/", "Games"),
            ("C:\\Users\\player\\My Games\\saves\\level.dat", "level.dat"),
            (
                "/home/player/My Games/access_token=abc",
                "access_token=[redacted]",
            ),
        ] {
            assert_eq!(applied(raw), expected, "{raw}");
        }
    }

    #[test]
    fn following_a_path_stops_where_the_path_does() {
        for (raw, expected) in [
            (
                "read /home/player/saves for writing",
                "read saves for writing",
            ),
            (
                "read /home/player/saves https://example.invalid/x",
                "read saves https://example.invalid/x",
            ),
            ("/home/player/saves\nGames/mods", "saves\nGames/mods"),
            ("(/home/player/logs) Games/mods", "(logs) Games/mods"),
        ] {
            assert_eq!(applied(raw), expected, "{raw}");
        }
    }

    #[test]
    fn a_token_is_removed_on_its_own_shape_with_no_word_or_prefix_to_announce_it() {
        let token = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJyYXZlbCJ9.dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1g";

        assert_eq!(applied(token), "[redacted]");
        assert_eq!(
            applied(&format!("the launcher passed {token} to the game")),
            "the launcher passed [redacted] to the game"
        );
        assert_eq!(applied(&format!("`{token}`")), "`[redacted]`");
        assert_eq!(applied(&format!("/home/player/{token}")), "[redacted]");
    }

    #[test]
    fn a_dotted_name_that_is_not_a_token_survives() {
        for text in [
            "1.0.0",
            "com.example.mod",
            "net.minecraft.util",
            "level.dat.backup",
            "dev.ravel.jvm.Hook",
            "libravel_bootstrap.so",
        ] {
            assert_eq!(applied(text), text);
        }
    }

    #[test]
    fn a_relative_path_and_a_url_are_left_alone() {
        for text in [
            "the package declares `src/lib.rs` but does not contain it",
            "the package declares `token.json` but does not contain it",
            "see https://example.invalid/example_mod for the source",
            "expected value at line 1 column 1",
            "`example_mod` requires `example_library` `^1.0.0`, which is not installed",
        ] {
            assert_eq!(applied(text), text);
        }
    }

    #[test]
    fn a_credential_value_is_removed_rather_than_shortened() {
        for (raw, expected) in [
            ("access_token=abc123", "access_token=[redacted]"),
            (
                "MINECRAFT_ACCESS_TOKEN=abc",
                "MINECRAFT_ACCESS_TOKEN=[redacted]",
            ),
            ("password=hunter2", "password=[redacted]"),
            (
                "authorization: Bearer abc.def",
                "authorization: Bearer [redacted]",
            ),
            ("Bearer abc.def", "Bearer [redacted]"),
            (
                "the password hunter2 was rejected",
                "the password [redacted] was rejected",
            ),
            ("ghp_0123456789", "[redacted]"),
            ("xoxb-1-2-3", "[redacted]"),
            (
                "read /home/player/.minecraft/access_token=abc",
                "read access_token=[redacted]",
            ),
            ("/home/player/ghp_0123456789", "[redacted]"),
        ] {
            assert_eq!(applied(raw), expected, "{raw}");
        }
    }

    #[test]
    fn a_quoted_word_keeps_its_punctuation_and_loses_its_path() {
        assert_eq!(
            applied("`/home/player/instance/mods`, which is not readable"),
            "`mods`, which is not readable"
        );
        assert_eq!(applied("(/home/player/logs)"), "(logs)");
    }

    #[test]
    fn whitespace_and_ordering_survive_the_policy() {
        assert_eq!(
            applied("  two   spaces\nand a line "),
            "  two   spaces\nand a line "
        );
        assert_eq!(applied(""), "");
        assert_eq!(applied("   "), "   ");
        assert_eq!(
            applied("`` -- \"...\""),
            "`` -- \"...\"",
            "a word that is only punctuation"
        );
    }

    #[test]
    fn the_policy_is_the_only_way_to_obtain_an_applied_message() {
        let raw = DiagnosticMessage::raw("/home/player/token.txt");
        assert_eq!(raw.redaction(), Redaction::Raw);
        assert!(!raw.is_publishable());

        let published = DiagnosticMessage::redacted(&Redactor, raw.text());
        assert_eq!(published, redact(raw.text()));
        assert_eq!(published.text(), "token.txt");
        assert_eq!(Redactor.apply(raw.text()), "token.txt");
    }
}
