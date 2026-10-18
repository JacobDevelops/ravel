//! Hook inventory parsing and derived counts shared by repository checks.

use std::path::Path;

use serde::Deserialize;

use crate::json::read_text;

pub(crate) struct HookInventory {
    pub(crate) version: String,
    pub(crate) path: String,
    pub(crate) text: String,
}

pub(crate) fn hook_inventories(root: &Path, versions: &[String]) -> Vec<HookInventory> {
    versions
        .iter()
        .filter_map(|version| {
            let path = format!("minecraft/versions/{version}/hooks.toml");
            let text = read_text(root, Path::new(&path)).ok()?;
            Some(HookInventory {
                version: version.clone(),
                path,
                text,
            })
        })
        .collect()
}

#[derive(Deserialize)]
pub(crate) struct DeclaredHooks {
    #[serde(default, rename = "hook")]
    pub(crate) hooks: Vec<HookEntry>,
    #[serde(default, rename = "nonGoal")]
    pub(crate) non_goals: Vec<toml::Value>,
}

#[derive(Deserialize)]
pub(crate) struct HookEntry {
    pub(crate) id: String,
    pub(crate) kind: Option<String>,
    pub(crate) descriptor: Option<String>,
}

pub(crate) fn parse_hook_inventory(text: &str) -> Result<DeclaredHooks, String> {
    toml::from_str(text).map_err(|error| format!("cannot parse hook inventory as TOML: {error}"))
}

pub(crate) struct HookCounts {
    pub(crate) entries: usize,
    pub(crate) methods: usize,
    pub(crate) fields: usize,
    pub(crate) handles: usize,
}

pub(crate) fn hook_counts(inventory: &str) -> Result<HookCounts, String> {
    let mut counts = HookCounts {
        entries: 0,
        methods: 0,
        fields: 0,
        handles: 0,
    };
    for hook in parse_hook_inventory(inventory)?.hooks {
        let (Some(kind), Some(descriptor)) = (hook.kind.as_deref(), hook.descriptor.as_deref())
        else {
            return Err("a `[[hook]]` entry declares no kind or no descriptor".to_owned());
        };
        counts.entries += 1;
        if kind.ends_with("-field") {
            counts.fields += 1;
        } else {
            counts.methods += 1;
            counts.handles += ravel_minecraft_generate::descriptor_handle_count(descriptor)
                .map_err(|error| format!("`{descriptor}`: {error}"))?;
        }
    }
    if counts.entries == 0 {
        return Err("the inventory declares no `[[hook]]` entry".to_owned());
    }
    Ok(counts)
}
