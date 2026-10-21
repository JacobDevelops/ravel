//! Typed generator-owned hook inventory facts and repository-derived counts.

pub(crate) use ravel_minecraft_generate::HookInventory;
#[cfg(test)]
pub(crate) use ravel_minecraft_generate::hook_inventory;

pub(crate) struct HookCounts {
    pub(crate) entries: usize,
    pub(crate) methods: usize,
    pub(crate) fields: usize,
    pub(crate) handles: usize,
}

pub(crate) fn hook_counts(inventory: &HookInventory) -> Result<HookCounts, String> {
    let mut counts = HookCounts {
        entries: inventory.hooks.len(),
        methods: 0,
        fields: 0,
        handles: 0,
    };
    for hook in &inventory.hooks {
        if hook.kind.is_method() {
            counts.methods += 1;
            counts.handles += ravel_minecraft_generate::descriptor_handle_count(&hook.descriptor)
                .map_err(|error| format!("`{}`: {error}", hook.descriptor))?;
        } else {
            counts.fields += 1;
        }
    }
    if counts.entries == 0 {
        return Err("the inventory declares no `[[hook]]` entry".to_owned());
    }
    Ok(counts)
}
