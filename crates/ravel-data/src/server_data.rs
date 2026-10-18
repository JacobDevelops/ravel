use std::fmt;

/// A kind of generated entry resolved by the server.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ServerDataKind {
    /// A conversion from some items to others.
    Crafting,
    /// A table of what something drops.
    Loot,
    /// A step in a player's recorded progression.
    Progression,
    /// A named sequence of commands the server runs.
    Procedure,
}

impl ServerDataKind {
    /// Every kind, in the order this enum declares them.
    pub const ALL: [Self; 4] = [
        Self::Crafting,
        Self::Loot,
        Self::Progression,
        Self::Procedure,
    ];

    /// Returns the kind's canonical lowercase name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Crafting => "crafting",
            Self::Loot => "loot",
            Self::Progression => "progression",
            Self::Procedure => "procedure",
        }
    }

    /// Returns the canonical pack directory adapters read this kind from.
    #[must_use]
    pub const fn directory(self) -> &'static str {
        match self {
            Self::Crafting => "recipe",
            Self::Loot => "loot_table",
            Self::Progression => "advancement",
            Self::Procedure => "function",
        }
    }

    /// Returns the extension this kind's generated files carry.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Crafting | Self::Loot | Self::Progression => "json",
            Self::Procedure => "mcfunction",
        }
    }
}

impl fmt::Display for ServerDataKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{ContentKind, ModId, RelativePath};

    use crate::{ContentDeclaration, ContentDeclarationError};

    use super::ServerDataKind;

    const BODY: &[u8] = br#"{"result": "sample:plate"}"#;

    fn owner() -> ModId {
        "sample".parse().expect("a well formed owner id")
    }

    fn declare(order: [&str; 3]) -> Vec<(String, Vec<u8>)> {
        let mut declared = ContentDeclaration::new(owner());
        for name in order {
            declared
                .server_entry(ServerDataKind::Crafting, name, BODY)
                .expect("each name is claimed once");
        }
        declared
            .generate()
            .files()
            .iter()
            .map(|file| (file.path().to_string(), file.bytes().to_vec()))
            .collect()
    }

    #[test]
    fn a_declared_server_entry_reaches_generation() {
        let mut declared = ContentDeclaration::new(owner());
        let path = declared
            .server_entry(ServerDataKind::Crafting, "plate_from_ingot", BODY)
            .expect("a first claim on an unclaimed name is taken");
        let generated = declared.generate();
        assert_eq!(generated.len(), 1);
        assert_eq!(
            generated
                .file(&path)
                .expect("the file is generated")
                .bytes(),
            BODY
        );
        assert!(
            declared.entries().is_empty(),
            "data files are not registry claims"
        );
    }

    #[test]
    fn generation_of_nothing_is_empty_rather_than_refused() {
        let generated = ContentDeclaration::new(owner()).generate();
        assert_eq!(generated.files().len(), 0);
        assert!(generated.is_empty());
        assert_eq!(generated.len(), 0);
    }

    #[test]
    fn every_kind_is_generated_at_the_path_this_test_spells_out() {
        let mut declared = ContentDeclaration::new(owner());
        let expected = [
            (ServerDataKind::Crafting, "data/sample/recipe/thing.json"),
            (ServerDataKind::Loot, "data/sample/loot_table/thing.json"),
            (
                ServerDataKind::Progression,
                "data/sample/advancement/thing.json",
            ),
            (
                ServerDataKind::Procedure,
                "data/sample/function/thing.mcfunction",
            ),
        ];
        for (kind, expected) in expected {
            let path = declared
                .server_entry(kind, "thing", BODY)
                .expect("one name under four kinds is four files");
            assert_eq!(path.as_str(), expected);
            assert_eq!(kind.to_string(), kind.as_str());
        }
        assert_eq!(declared.generate().len(), 4);
    }

    #[test]
    fn a_nested_name_keeps_its_segments_and_gains_the_kinds_extension() {
        let mut declared = ContentDeclaration::new(owner());
        let path = declared
            .server_entry(ServerDataKind::Crafting, "smelting/plate", BODY)
            .expect("a nested name is a legal content path");
        assert_eq!(path.as_str(), "data/sample/recipe/smelting/plate.json");
    }

    #[test]
    fn server_entries_and_resources_share_one_collision_barrier() {
        let path = RelativePath::try_from("data/sample/recipe/plate.json".to_owned())
            .expect("a legal output path");
        for resource_first in [false, true] {
            let mut declared = ContentDeclaration::new(owner());
            if resource_first {
                declared
                    .resource(path.clone(), BODY.to_vec())
                    .expect("first claim");
            } else {
                declared
                    .server_entry(ServerDataKind::Crafting, "plate", BODY)
                    .expect("first claim");
            }
            let before = declared.clone();
            let refusal = declared
                .server_entry(ServerDataKind::Crafting, "plate", b"{}")
                .expect_err("a repeated file is refused");
            assert_eq!(
                refusal,
                ContentDeclarationError::DuplicateResource { path: path.clone() }
            );
            assert_eq!(
                refusal.to_string(),
                "this mod already supplies `data/sample/recipe/plate.json`, and one path holds one file"
            );
            assert_eq!(declared, before, "refusal leaves the declaration unchanged");
            let refusal = declared
                .resource(path.clone(), b"{}".to_vec())
                .expect_err("a resource cannot replace the claimed file");
            assert_eq!(
                refusal,
                ContentDeclarationError::DuplicateResource { path: path.clone() }
            );
            assert_eq!(
                refusal.to_string(),
                "this mod already supplies `data/sample/recipe/plate.json`, and one path holds one file"
            );
            assert_eq!(declared, before, "refusal leaves the declaration unchanged");
        }
    }

    #[test]
    fn invalid_names_are_refused_without_mutation() {
        let mut declared = ContentDeclaration::new(owner());
        for name in ["Uppercase", "../elsewhere", ""] {
            let refusal = declared
                .server_entry(ServerDataKind::Crafting, name, BODY)
                .expect_err("an invalid content name is refused");
            assert!(matches!(refusal, ContentDeclarationError::Path { .. }));
            assert!(declared.generate().is_empty());
        }
    }

    #[test]
    fn generated_bytes_do_not_depend_on_declaration_order_or_run() {
        let forward = declare(["first", "second", "third"]);
        assert_eq!(forward, declare(["third", "second", "first"]));
        assert_eq!(forward, declare(["first", "second", "third"]));
    }

    #[test]
    fn a_server_loot_entry_replaces_the_conventional_block_default() {
        let mut declared = ContentDeclaration::new(owner());
        declared
            .entry(ContentKind::Block, "plate")
            .expect("a block");
        let path = declared
            .server_entry(ServerDataKind::Loot, "blocks/plate", BODY)
            .expect("a supplied loot table replaces the default");
        assert_eq!(declared.supplied_defaults(), std::slice::from_ref(&path));
        assert_eq!(
            declared
                .generate()
                .file(&path)
                .expect("loot exists")
                .bytes(),
            BODY
        );
    }
}
