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
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Generated pack files in unique path order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GeneratedData {
    files: Vec<GeneratedFile>,
}

impl GeneratedData {
    /// Records `bytes` as the content of `path`, keeping the files in path order.
    ///
    /// A repeated path replaces its earlier bytes.
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

/// Renders sorted-key, pretty JSON with a trailing newline.
fn document(value: &Value) -> Vec<u8> {
    let mut text = serde_json::to_string_pretty(value)
        .expect("a map of strings and arrays of strings always serializes");
    text.push('\n');
    text.into_bytes()
}

/// Renders tag members with `replace: false` to preserve other contributions.
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

/// Renders the empty-variant blockstate mapping.
pub(crate) fn blockstate_document(namespace: &str, path: &str) -> Vec<u8> {
    let mut variant = Map::new();
    variant.insert(
        "model".to_owned(),
        Value::String(format!("{namespace}:block/{path}")),
    );
    let mut variants = Map::new();
    variants.insert(String::new(), Value::Object(variant));
    let mut object = Map::new();
    object.insert("variants".to_owned(), Value::Object(variants));
    document(&Value::Object(object))
}

/// Renders a self-drop loot table with a survives-explosion condition.
pub(crate) fn loot_table_document(namespace: &str, path: &str) -> Vec<u8> {
    let mut condition = Map::new();
    condition.insert(
        "condition".to_owned(),
        Value::String("minecraft:survives_explosion".to_owned()),
    );
    let mut entry = Map::new();
    entry.insert(
        "type".to_owned(),
        Value::String("minecraft:item".to_owned()),
    );
    entry.insert(
        "name".to_owned(),
        Value::String(format!("{namespace}:{path}")),
    );
    let mut pool = Map::new();
    pool.insert(
        "bonus_rolls".to_owned(),
        Value::Number(serde_json::Number::from_f64(0.0).expect("zero is a number")),
    );
    pool.insert(
        "conditions".to_owned(),
        Value::Array(vec![Value::Object(condition)]),
    );
    pool.insert(
        "entries".to_owned(),
        Value::Array(vec![Value::Object(entry)]),
    );
    pool.insert(
        "rolls".to_owned(),
        Value::Number(serde_json::Number::from_f64(1.0).expect("one is a number")),
    );
    let mut object = Map::new();
    object.insert("pools".to_owned(), Value::Array(vec![Value::Object(pool)]));
    object.insert(
        "type".to_owned(),
        Value::String("minecraft:block".to_owned()),
    );
    document(&Value::Object(object))
}

/// Renders an item model that shows a block.
pub(crate) fn item_model_document(model: &str) -> Vec<u8> {
    let mut object = Map::new();
    object.insert("parent".to_owned(), Value::String(model.to_owned()));
    document(&Value::Object(object))
}

/// Renders an item model that shows a flat sprite.
pub(crate) fn sprite_model_document(texture: &str) -> Vec<u8> {
    let mut textures = Map::new();
    textures.insert("layer0".to_owned(), Value::String(texture.to_owned()));
    let mut object = Map::new();
    object.insert(
        "parent".to_owned(),
        Value::String("minecraft:item/generated".to_owned()),
    );
    object.insert("textures".to_owned(), Value::Object(textures));
    document(&Value::Object(object))
}

/// Renders the item definition a newer version resolves a model through.
pub(crate) fn item_definition_document(model: &str) -> Vec<u8> {
    let mut named = Map::new();
    named.insert("model".to_owned(), Value::String(model.to_owned()));
    named.insert(
        "type".to_owned(),
        Value::String("minecraft:model".to_owned()),
    );
    let mut object = Map::new();
    object.insert("model".to_owned(), Value::Object(named));
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
        assert_eq!(data.files(), []);
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
