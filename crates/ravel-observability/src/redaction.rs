use ravel_core::{DiagnosticMessage, RedactionPolicy};

/// What replaces a value the policy removes outright.
const REDACTED: &str = "[redacted]";

/// The words that make a name a credential's name.
///
/// This is the set `tools/ravel-xtask` rejects as a key in a recorded evidence
/// manifest, deliberately: a report that would be rejected there is a report
/// that should never have been published here either.
const CREDENTIAL_WORDS: [&str; 5] = ["token", "password", "secret", "credential", "authorization"];

/// Value prefixes that identify a credential whatever it sits next to.
const CREDENTIAL_PREFIXES: [&str; 6] = ["ghp_", "gho_", "ghs_", "xoxb-", "xoxp-", "sk-"];

/// How long each of a token's three segments has to be before the dotted shape
/// is read as a credential rather than as a name.
///
/// A version (`1.0.0`), a namespaced id (`com.example.mod`) and a doubly
/// suffixed file (`level.dat.backup`) are all three dot-separated runs of
/// base64url characters. None of them reaches this length in every segment, and
/// the shortest real header segment — the encoding of `{"alg":"HS256"}` — is
/// well past it.
const TOKEN_SEGMENT: usize = 8;

/// Punctuation a message wraps a word in, which is kept rather than redacted.
const OPENING: [char; 5] = ['`', '\'', '"', '(', '['];
const CLOSING: [char; 9] = ['`', '\'', '"', ')', ']', '.', ',', ';', '!'];

/// The rules `SPEC.md` §9.1 requires of every published Ravel report.
///
/// Two things come out of a message: a filesystem path becomes the file it
/// names, and a credential's value becomes nothing at all.
///
/// The first rule is the one the rest of the workspace already models.
/// `ravel_package::RelativePath::file_name` exists because an integrity failure
/// is about a file inside a package while the directory that package was
/// installed into says where a player keeps their game. This policy applies the
/// same reduction to text nobody in this workspace wrote — the sentence a mod
/// hands back when it declines a lifecycle step is the one message in the
/// pre-world path that can hold anything at all.
///
/// A directory name may contain a space, so a path is not one whitespace-
/// delimited word: the reduction follows a path across the spaces inside it and
/// publishes the file the whole run names.
///
/// The second rule is unconditional. `SPEC.md` §10 requires that logs and
/// diagnostics never expose launcher tokens, and a credential has no shortened
/// form worth keeping, so its value is replaced rather than reduced. A token
/// need not be announced to be recognised: an account token arrives from a
/// launcher with no credential word beside it and no vendor prefix on it, and
/// its three-segment shape is what identifies it.
///
/// Both rules over-redact rather than under-redact where they are unsure: a
/// bare `token` or `password` in prose costs the word after it. Rules this
/// policy deliberately does not have: it does not shorten a URL, because a
/// package's `source` is published metadata, and it does not touch a mod id, a
/// version, or a lifecycle state, because those are what a report is for.
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
///
/// This is the only way to obtain [`ravel_core::Redaction::Applied`] anywhere in
/// the workspace. `ravel-core` holds the tag and no rules, this crate holds the
/// rules, and `DiagnosticMessage::redacted` runs them rather than taking a
/// caller's word that they ran.
#[must_use]
pub fn redact(text: &str) -> DiagnosticMessage {
    DiagnosticMessage::redacted(&Redactor, text)
}

/// Splits the next whitespace-delimited word from the text that follows it.
fn split_word(rest: &str) -> (&str, &str) {
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    rest.split_at(end)
}

/// Follows an absolute path across the spaces a directory name can contain.
///
/// A fragment continues the path before it when it carries a separator and does
/// not start somewhere of its own, and the run then reduces to the file the last
/// fragment names rather than leaving the tail of a player's path in the
/// message. The run stops at a line break, at punctuation that closed the path,
/// and at a URL, because none of those is the middle of a path — and stopping
/// early only costs a component that names no player.
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

/// Returns whether this fragment is the middle of the path before it.
fn continues_a_path(core: &str) -> bool {
    core.contains(separator) && !is_absolute(core) && !core.contains("://")
}

/// Splits the punctuation a message wraps a word in from the word itself.
fn undecorate(word: &str) -> (&str, &str, &str) {
    let unopened = word.trim_start_matches(OPENING);
    let opening = &word[..word.len() - unopened.len()];
    let core = unopened.trim_end_matches(CLOSING);
    let closing = &word[opening.len() + core.len()..];
    (opening, core, closing)
}

/// Applies both rules to one undecorated word.
///
/// The path rule runs first, because a word can be both: a file inside a
/// player's home directory can be named after the credential it holds, and
/// reducing the path first leaves the credential rule something to remove.
fn redact_word(core: &str, value_follows: bool) -> String {
    if core.is_empty() {
        return String::new();
    }
    // A credential's name is not its value, so the name survives and the word
    // after it does not.
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

/// Returns whether this word is a credential's name, so the next word is its
/// value.
///
/// The match is exact rather than a substring, because `token.json` is a file
/// and `access_token=…` already carries its own value: treating either as a
/// label would cost the innocent word after it.
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

/// Returns whether this word is a token on its own evidence.
///
/// Three base64url segments separated by dots is what a signed token looks
/// like, and a launcher hands one over with nothing beside it to say so. The
/// per-segment length is what keeps a version or a namespaced id out; anything
/// that reaches it and is still not a credential costs a reader one unreadable
/// word, which is the side to be wrong on.
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

/// Returns the file an absolute path names, and nothing for anything else.
///
/// A path is absolute when it starts at a root: a Unix root, a home shorthand,
/// a Windows drive, or a share. A package-relative path is none of those and is
/// left alone, because it names a file inside a package rather than a place on
/// the player's disk.
fn file_name_of(core: &str) -> Option<&str> {
    if !is_absolute(core) {
        return None;
    }
    Some(last_component(core))
}

/// The last non-empty component of a path, or nothing nameable at all.
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
            // A word with no separator is prose, not the middle of a path.
            (
                "read /home/player/saves for writing",
                "read saves for writing",
            ),
            // A URL is published metadata and starts nothing.
            (
                "read /home/player/saves https://example.invalid/x",
                "read saves https://example.invalid/x",
            ),
            // A path does not span a line break.
            ("/home/player/saves\nGames/mods", "saves\nGames/mods"),
            // Punctuation already closed the path.
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
            "the package declares `artifacts/mod.bin` but does not contain it",
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
