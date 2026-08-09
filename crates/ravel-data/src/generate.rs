use std::collections::BTreeMap;

use ravel_core::RelativePath;
use serde_json::{Map, Value};

/// One file a declaration produces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedFile {
    path: RelativePath,
    bytes: Vec<u8>,
}

impl GeneratedFile {
    /// Returns where the file belongs in the generated pack.
    #[must_use]
    pub const fn path(&self) -> &RelativePath {
        &self.path
    }

    /// Returns the file's exact content.
    ///
    /// Bytes rather than text, because a mod may supply a texture and because
    /// the property this whole surface is judged on is byte equality. A `String`
    /// would invite comparing values that render the same and differ on disk.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// The pack a declaration produces.
///
/// The files are ordered by path and each path appears once, which is the whole
/// of the determinism claim: two runs over the same declaration emit the same
/// files in the same order with the same bytes, so the output can be compared
/// byte for byte rather than field by field. Nothing here reads a clock, a
/// filesystem, an environment variable or a hash map's iteration order, and
/// [`crate::ContentDeclaration`] keeps every collection it generates from keyed
/// or sorted by content rather than by declaration order.
///
/// `tests/conformance/tests/content_declarations.rs` is where that is checked
/// rather than asserted: it generates the same declaration twice, builds the
/// same content in two different declaration orders, and requires the bytes to
/// match.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GeneratedData {
    files: Vec<GeneratedFile>,
}

impl GeneratedData {
    /// Records `bytes` as the content of `path`, keeping the files in path order.
    ///
    /// A repeated path replaces the earlier bytes, and no caller can reach that:
    /// [`crate::ContentDeclaration`] refuses two producers for one path when the
    /// declaration is made, so a collision cannot survive to generation.
    pub(crate) fn insert(&mut self, path: RelativePath, bytes: Vec<u8>) {
        match self
            .files
            .binary_search_by(|file| file.path.as_str().cmp(path.as_str()))
        {
            Ok(existing) => self.files[existing].bytes = bytes,
            Err(position) => self.files.insert(position, GeneratedFile { path, bytes }),
        }
    }

    /// Returns every generated file, in path order.
    #[must_use]
    pub fn files(&self) -> &[GeneratedFile] {
        &self.files
    }

    /// Returns the paths of every generated file, in path order.
    pub fn paths(&self) -> impl Iterator<Item = &str> + '_ {
        self.files.iter().map(|file| file.path.as_str())
    }

    /// Returns the file at `path`, if the declaration produces one.
    #[must_use]
    pub fn file(&self, path: &RelativePath) -> Option<&GeneratedFile> {
        self.files
            .binary_search_by(|file| file.path.as_str().cmp(path.as_str()))
            .ok()
            .map(|found| &self.files[found])
    }

    /// Returns whether the declaration produces nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Returns how many files the declaration produces.
    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }
}

/// Renders one JSON document the way every generated file is rendered.
///
/// One function, so no two generated file kinds can disagree about formatting.
/// The rendering is `serde_json`'s pretty form plus a trailing newline: a JSON
/// file without one is a file every text tool reports as ending mid-line, and
/// the diff a player or an author reads is worth two bytes.
///
/// Object key order is not decided here and does not need to be. `serde_json`'s
/// `Map` is a `BTreeMap` unless its `preserve_order` feature is on, and this
/// workspace enables no features on it, so keys are emitted in sorted order by
/// construction rather than by a sort this file would have to remember to do.
fn document(value: &Value) -> Vec<u8> {
    let mut text = serde_json::to_string_pretty(value)
        .expect("a map of strings and arrays of strings always serializes");
    text.push('\n');
    text.into_bytes()
}

/// Renders one tag file from its members' canonical spellings.
///
/// `replace` is present and false. The pack format treats an absent `replace` as
/// false too, so this could be left out — and then a mod reading its own
/// generated tag would have to know that, and a mod that ever needs to replace a
/// tag would find the field missing rather than set. Ravel never generates
/// `true`: replacing another mod's tag members discards contributions the tag
/// exists to collect, and a pack that means to do it is M4-W5's business.
pub(crate) fn tag_document(members: &[String]) -> Vec<u8> {
    let mut object = Map::new();
    object.insert("replace".to_owned(), Value::Bool(false));
    object.insert(
        "values".to_owned(),
        Value::Array(
            members
                .iter()
                .map(|member| Value::String(member.clone()))
                .collect(),
        ),
    );
    document(&Value::Object(object))
}

/// Renders one locale's translations.
pub(crate) fn translation_document(entries: &BTreeMap<String, String>) -> Vec<u8> {
    let object: Map<String, Value> = entries
        .iter()
        .map(|(key, text)| (key.clone(), Value::String(text.clone())))
        .collect();
    document(&Value::Object(object))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ravel_core::RelativePath;

    use super::{GeneratedData, tag_document, translation_document};

    fn path(text: &str) -> RelativePath {
        RelativePath::try_from(text.to_owned()).expect("a legal relative path")
    }

    #[test]
    fn an_empty_pack_holds_no_file() {
        let data = GeneratedData::default();
        assert!(data.is_empty());
        assert_eq!(data.len(), 0);
        assert!(data.files().is_empty());
        assert_eq!(data.paths().count(), 0);
        assert_eq!(data.file(&path("anything.json")), None);
    }

    #[test]
    fn files_are_ordered_by_path_rather_than_by_the_order_they_were_recorded() {
        let mut data = GeneratedData::default();
        data.insert(path("data/z.json"), b"z".to_vec());
        data.insert(path("assets/a.json"), b"a".to_vec());

        assert_eq!(
            data.paths().collect::<Vec<_>>(),
            ["assets/a.json", "data/z.json"]
        );
        let files = data.files();
        assert_eq!(files[0].path().as_str(), "assets/a.json");
        assert_eq!(files[0].bytes(), b"a");
        assert_eq!(data.len(), 2);
    }

    #[test]
    fn a_tag_document_is_the_exact_text_this_test_pins() {
        let rendered = tag_document(&["example_mod:copper".to_owned(), "other:tin".to_owned()]);
        assert_eq!(
            String::from_utf8(rendered).expect("generated JSON is UTF-8"),
            "{\n  \"replace\": false,\n  \"values\": [\n    \"example_mod:copper\",\n    \
             \"other:tin\"\n  ]\n}\n"
        );
    }

    #[test]
    fn an_empty_tag_document_is_still_a_document_with_no_members() {
        let rendered = tag_document(&[]);
        assert_eq!(
            String::from_utf8(rendered).expect("generated JSON is UTF-8"),
            "{\n  \"replace\": false,\n  \"values\": []\n}\n"
        );
    }

    #[test]
    fn a_translation_document_emits_its_keys_in_sorted_order() {
        let mut entries = BTreeMap::new();
        entries.insert("zzz".to_owned(), "last".to_owned());
        entries.insert("aaa".to_owned(), "first".to_owned());

        assert_eq!(
            String::from_utf8(translation_document(&entries)).expect("generated JSON is UTF-8"),
            "{\n  \"aaa\": \"first\",\n  \"zzz\": \"last\"\n}\n"
        );
    }

    #[test]
    fn every_generated_document_ends_in_exactly_one_newline() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "a".to_owned(),
            "text with \"quotes\" and \\ escapes".to_owned(),
        );

        for rendered in [
            tag_document(&["a:b".to_owned()]),
            translation_document(&entries),
        ] {
            assert!(rendered.ends_with(b"\n"));
            assert!(!rendered.ends_with(b"\n\n"));
            // The escaping is serde_json's, so a translation holding a quote is
            // still one line of legal JSON.
            let text = String::from_utf8(rendered).expect("generated JSON is UTF-8");
            serde_json::from_str::<serde_json::Value>(&text).expect("legal JSON");
        }
    }
}
