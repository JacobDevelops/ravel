use crate::diagnostics::DiagnosticMessage;

const REDACTED: &str = "[redacted]";

/// Case-insensitive labels identifying credential-bearing assignments.
pub const CREDENTIAL_WORDS: [&str; 8] = [
    "token",
    "password",
    "secret",
    "credential",
    "authorization",
    "api_key",
    "access_token",
    "minecraft_access_token",
];

/// Case-sensitive prefixes identifying credential values without a label.
pub const CREDENTIAL_PREFIXES: [&str; 6] = ["ghp_", "gho_", "ghs_", "xoxb-", "xoxp-", "sk-"];

// JWTs have an encoded JSON header, not just three dotted identifiers.
const TOKEN_SEGMENT: usize = 8;

const OPENING: [char; 5] = ['`', '\'', '"', '(', '['];
const CLOSING: [char; 9] = ['`', '\'', '"', ')', ']', '.', ',', ';', '!'];

pub(crate) fn redact_text(text: &str) -> String {
    let mut redacted = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((start, end)) = credential_span(rest, false) {
        redact_paths(&rest[..start], &mut redacted);
        redacted.push_str(REDACTED);
        rest = &rest[end..];
    }
    redact_paths(rest, &mut redacted);
    redacted
}

fn redact_paths(mut rest: &str, redacted: &mut String) {
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
        redacted.push_str(file_name_of(core).unwrap_or(core));
        redacted.push_str(closing);
        rest = tail;
    }
}

/// Redacts `text` and returns it as a message that may be shown to a player.
#[must_use]
pub fn redact(text: &str) -> DiagnosticMessage {
    DiagnosticMessage::redacted(text)
}

/// Detects credential-shaped values while avoiding ambiguous source-code assignments.
#[must_use]
pub fn contains_credential(text: &str) -> bool {
    credential_span(text, true).is_some()
}

fn credential_span(text: &str, strict: bool) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if !token_character(bytes[cursor]) {
            cursor += 1;
            continue;
        }
        let start = cursor;
        while cursor < bytes.len() && (token_character(bytes[cursor]) || bytes[cursor] == b'.') {
            cursor += 1;
        }
        let token = &text[start..cursor];
        let shaped = token.trim_end_matches('.');
        if starts_with_a_credential_prefix(shaped) || has_the_shape_of_a_token(shaped) {
            return Some((credential_path_start(text, start), start + shaped.len()));
        }
        if let Some(value) = labelled_value(text, token, start, cursor, strict) {
            return Some(value);
        }
    }
    None
}

fn labelled_value(
    text: &str,
    label: &str,
    start: usize,
    end: usize,
    strict: bool,
) -> Option<(usize, usize)> {
    let tail = &text[end..];
    let bearer = label.eq_ignore_ascii_case("bearer");
    let value = if bearer {
        if !tail.starts_with([' ', '\t']) {
            return None;
        }
        tail.trim_start_matches([' ', '\t'])
    } else {
        if !names_a_credential(label) {
            return None;
        }
        let tail = tail
            .trim_start_matches(['"', '\''])
            .trim_start_matches([' ', '\t']);
        let value = if let Some(value) = tail.strip_prefix('=') {
            value
        } else {
            if text[..start].ends_with(':') {
                return None;
            }
            tail.strip_prefix(':')?
        };
        value.trim_start_matches([' ', '\t'])
    };
    if value.starts_with(':') {
        return None;
    }
    let bearer = bearer
        || (value
            .get(..6)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("bearer"))
            && value[6..].starts_with([' ', '\t']));
    let value = if value
        .get(..6)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("bearer"))
        && value[6..].starts_with([' ', '\t'])
    {
        value[6..].trim_start_matches([' ', '\t'])
    } else {
        value
    };
    // Scans qualify ambiguous assignments; player-facing redaction masks every
    // nonempty labelled value, retaining its full extent to avoid suffix leaks.
    if strict && !bearer && !value.starts_with(['"', '\'']) {
        let length = value
            .bytes()
            .take_while(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(*byte, b'_' | b'-' | b'.' | b'+' | b'/' | b'=' | b'~')
            })
            .count();
        if length < 4 || value[length..].starts_with('(') {
            return None;
        }
    }
    let (value, length) = credential_value(value, json_key(text, start, end));
    if length == 0 || (strict && !bearer && length < 4) || &value[..length] == REDACTED {
        return None;
    }
    let start = text.len() - value.len();
    Some((start, start + length))
}

fn credential_value(value: &str, json: bool) -> (&str, usize) {
    if value.strip_prefix(REDACTED).is_some_and(|tail| {
        tail.is_empty()
            || tail.starts_with(|character: char| {
                character.is_whitespace()
                    || matches!(character, ',' | ';' | '}' | ']' | ')' | '`' | '"' | '\'')
            })
    }) {
        return (value, REDACTED.len());
    }
    if let Some(quote @ ('"' | '\'')) = value.chars().next() {
        let value = &value[1..];
        let mut escaped = false;
        for (index, character) in value.char_indices() {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == quote {
                return (value, index);
            }
        }
        return (value, value.len());
    }
    let end = value
        .find(|character: char| {
            character.is_whitespace() || (json && matches!(character, ',' | '}' | ']'))
        })
        .unwrap_or(value.len());
    (value, end)
}

fn json_key(text: &str, start: usize, end: usize) -> bool {
    text[..start]
        .strip_suffix('"')
        .is_some_and(|before| before.trim_end().ends_with(['{', ',']))
        && text[end..].starts_with('"')
}

fn credential_path_start(text: &str, start: usize) -> usize {
    let mut rest = text;
    while text.len() - rest.len() <= start && !rest.is_empty() {
        rest = rest.trim_start();
        let position = text.len() - rest.len();
        let (word, tail) = split_word(rest);
        let (opening, core, closing) = undecorate(word);
        let (_, _, after) = follow_a_path_across_spaces(core, closing, tail);
        if is_absolute(core) && start < text.len() - after.len() {
            return position + opening.len();
        }
        rest = after;
    }
    start
}

fn token_character(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
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

fn names_a_credential(name: &str) -> bool {
    if CREDENTIAL_WORDS
        .iter()
        .copied()
        .chain(["apikey", "credentials", "secrets", "passwords"])
        .any(|word| {
            name.as_bytes()
                .get(name.len().saturating_sub(word.len())..)
                .is_some_and(|suffix| suffix.eq_ignore_ascii_case(word.as_bytes()))
        })
    {
        return true;
    }
    let mut components = name.rsplit(['_', '-', '.']);
    components
        .next()
        .is_some_and(|word| word.eq_ignore_ascii_case("key"))
        && components
            .next()
            .is_some_and(|word| word.eq_ignore_ascii_case("api"))
}

fn starts_with_a_credential_prefix(core: &str) -> bool {
    CREDENTIAL_PREFIXES.iter().any(|prefix| {
        core.strip_prefix(prefix).is_some_and(|tail| {
            tail.as_bytes()
                .first()
                .is_some_and(|byte| token_character(*byte))
        })
    })
}

// An encoded JSON header starts with `eyJ`; ordinary dotted names do not.
fn has_the_shape_of_a_token(core: &str) -> bool {
    let mut segments = core.split('.');
    let shape = [segments.next(), segments.next(), segments.next()];
    core.starts_with("eyJ")
        && segments.next().is_none()
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
    use crate::diagnostics::{DiagnosticMessage, Redaction};

    use super::redact;

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
                "/home/player/My Games/access_token=abc123",
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
            "declaration.versions.contains",
            "delivery.direction.may_receive_on",
            "inventory.non_goals.is_empty",
            "TimeUnit.NANOSECONDS.toMillis",
            "environment.settings.modExecutionModelSelected",
            "worldPersistenceRoundTrip.worldRemovedAfterwards.whatSurvivesTheRemoval",
            "abcdefgh.ijklmnop.qrstuvwx",
        ] {
            assert_eq!(applied(text), text);
            assert!(!super::contains_credential(text), "{text}");
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
                "MINECRAFT_ACCESS_TOKEN=abc123",
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
                "the password hunter2 was rejected",
            ),
            (concat!("ghp_", "0123456789"), "[redacted]"),
            (concat!("xoxb-", "1-2-3"), "[redacted]"),
            (
                "read /home/player/.minecraft/access_token=abc123",
                "read access_token=[redacted]",
            ),
            (concat!("/home/player/ghp_", "0123456789"), "[redacted]"),
        ] {
            assert_eq!(applied(raw), expected, "{raw}");
        }
    }

    #[test]
    fn credential_detection_uses_the_publication_rules() {
        for text in [
            "Bearer abc",
            "authorization: Bearer xyz",
            "password=xyz123",
            "api_key: xyz123",
            "{\"token\":\"xyz123\"}",
            "{\"password\": \"xyz123\"}",
            "token = xyz123",
            "access_token=abc123",
            concat!("`ghp_", "0123456789`"),
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJyYXZlbCJ9.dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1g",
            "/home/player/password=secret",
        ] {
            assert!(super::contains_credential(text), "{text}");
        }
        for text in [
            "1.2.3",
            "https://example.test",
            "assets/token.txt",
            "hello",
            "Bearer",
            "authorization:",
            "token",
            "password",
            "the password was rejected",
            "token_count",
            "password=",
            "{\"token\":\"\"}",
            "ghp_",
            "sk-",
        ] {
            assert!(!super::contains_credential(text), "{text}");
            assert_eq!(applied(text), text, "{text}");
        }
    }

    #[test]
    fn every_secret_prefix_requires_a_value() {
        for prefix in super::CREDENTIAL_PREFIXES {
            assert!(!super::contains_credential(prefix), "{prefix}");
            let value = format!("{prefix}example");
            assert!(super::contains_credential(&value), "{prefix}");
            assert_eq!(applied(&value), "[redacted]");
        }
    }

    #[test]
    fn compact_and_spaced_assignments_redact_the_same_detected_values() {
        for (raw, expected) in [
            ("{\"token\":\"abc123\"}", "{\"token\":\"[redacted]\"}"),
            (
                "{\"password\": \"xyz123\"}",
                "{\"password\": \"[redacted]\"}",
            ),
            ("api_key: xyz123", "api_key: [redacted]"),
            ("token = xyz123", "token = [redacted]"),
        ] {
            assert_eq!(applied(raw), expected, "{raw}");
        }
    }

    #[test]
    fn credential_vocabulary_in_identifiers_is_not_an_assignment() {
        for text in [
            "failed to read tokens.json: permission denied",
            "token_count: 3",
            "max_tokens: 4096",
            "max_tokens=4096",
            "maxTokens: 4096",
            "maxTokens=4096",
            "MYTOKEN",
            "token::Kind",
            "`block:secretroomsmod:torch_lever`",
        ] {
            assert!(!super::contains_credential(text), "{text}");
            assert_eq!(applied(text), text, "{text}");
        }
        let path = "/home/player/.config/secrets.toml: No such file";
        assert!(!super::contains_credential(path));
        assert_eq!(applied(path), "secrets.toml: No such file");
    }

    #[test]
    fn source_syntax_and_offline_placeholders_are_not_detected_credentials() {
        for text in [
            "injected a `password=` assignment into a record",
            "String token = token(value, aliases);",
            concat!("[ -n \"$", "{token:-}\" ] || continue"),
            "'auth_uuid': offline, 'auth_access_token': '0', 'user_type': 'legacy',",
        ] {
            assert!(!super::contains_credential(text), "{text}");
        }
    }

    #[test]
    fn ambiguous_values_are_still_fully_redacted_from_player_messages() {
        for value in [
            "p@ssw0rd",
            "!hunter22",
            "Pass(word)1",
            "abc",
            "p,a,ss",
            "it's",
        ] {
            let raw = format!("password={value}");
            assert_eq!(applied(&raw), "password=[redacted]", "{raw}");
        }
    }

    #[test]
    fn compound_labels_match_credential_components_not_substrings() {
        for label in [
            "DB_PASSWORD",
            "client_secret",
            "refresh_token",
            "GITHUB_TOKEN",
            "X-Auth-Token",
            "auth_token",
            "spring.datasource.password",
            "minecraft.password",
            "service.api_key",
            "service-api-key",
            "accessToken",
            "clientToken",
            "clientSecret",
            "userPassword",
            "apiKey",
            "serviceApiKey",
            "MYTOKEN",
            "authtoken",
            "ACCESSTOKEN",
            "apitoken",
            "credentials",
            "secrets",
            "passwords",
        ] {
            for raw in [
                format!("{label}=example"),
                format!("{label}: example"),
                format!("{{\"{label}\":\"example\"}}"),
            ] {
                assert!(super::contains_credential(&raw), "{raw}");
                let published = applied(&raw);
                assert!(!published.contains("example"), "{published}");
                assert!(published.contains(label), "{published}");
                assert_eq!(applied(&published), published);
            }
        }
    }

    #[test]
    fn namespaced_assignments_preserve_namespace_but_remove_the_value() {
        for (raw, expected) in [
            ("env:DB_PASSWORD=example", "env:DB_PASSWORD=[redacted]"),
            ("minecraft:token: unknown", "minecraft:token: unknown"),
        ] {
            assert_eq!(applied(raw), expected);
        }
    }

    #[test]
    fn empty_assignments_do_not_consume_the_next_line() {
        for raw in [
            "password=\nordinary message",
            "token: \t\r\nordinary message",
            "Bearer\nordinary message",
            "password\n=ordinary message",
        ] {
            assert!(!super::contains_credential(raw), "{raw}");
            assert_eq!(applied(raw), raw);
        }
    }

    #[test]
    fn credential_values_keep_their_full_quoted_or_unquoted_extent() {
        for (raw, expected) in [
            ("password=hunter2;x", "password=[redacted]"),
            ("password=pass,a,ss", "password=[redacted]"),
            ("password=itsvalue's", "password=[redacted]"),
            (
                "password=\"correct horse battery\"",
                "password=\"[redacted]\"",
            ),
            (
                "{\"password\":\"correct horse battery\",\"token\":\"abc123\"}",
                "{\"password\":\"[redacted]\",\"token\":\"[redacted]\"}",
            ),
            (
                "{\"password\":\"a\\\"b c\"}",
                "{\"password\":\"[redacted]\"}",
            ),
            (
                "{\"token\":1234,\"other\":4}",
                "{\"token\":[redacted],\"other\":4}",
            ),
        ] {
            assert!(super::contains_credential(raw), "{raw}");
            assert_eq!(applied(raw), expected, "{raw}");
            assert_eq!(applied(expected), expected, "{raw}");
        }
    }

    #[test]
    fn shaped_credentials_replace_private_paths_even_across_spaces() {
        for path in [
            concat!("C:\\Users\\Player Name\\ghp_", "example"),
            "/home/player name/eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJyYXZlbCJ9.dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1g",
        ] {
            assert_eq!(applied(path), "[redacted]");
        }
    }

    #[test]
    fn applying_redaction_twice_preserves_markers_and_punctuation() {
        for text in [
            "token=abc123",
            "Bearer abc",
            "{\"token\":\"abc123\"}",
            "password='correct horse'",
            concat!("prefix `ghp_", "example` suffix"),
        ] {
            let once = applied(text);
            assert!(!super::contains_credential(&once), "{once}");
            assert_eq!(applied(&once), once);
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

        let published = DiagnosticMessage::redacted(raw.text());
        assert_eq!(published, redact(raw.text()));
        assert_eq!(published.text(), "token.txt");
        assert_eq!(super::redact_text(raw.text()), "token.txt");
    }
}
