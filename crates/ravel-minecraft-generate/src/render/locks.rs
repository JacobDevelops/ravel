//! Renders mapping and stub provenance without changing inventory order.

use std::fmt::Write as _;

use ravel_core::Sha256Digest;

use crate::EXTRACTOR;
use crate::inputs::Adapter;
use crate::resolve::Resolved;
use crate::stubs::Stub;

/// Leaves an absent hash absent instead of recording an empty-string identity.
fn hash_pin(text: &mut String, name: &str, digest: Option<Sha256Digest>) {
    if let Some(digest) = digest {
        writeln!(text, "{name} = \"{digest}\"").expect("writing to a String cannot fail");
    }
}

/// Renders the mapping provenance lock.
pub(crate) fn lock(adapter: &Adapter, resolved: &[Resolved]) -> String {
    let mut text = String::with_capacity(resolved.len() * 192 + 512);
    writeln!(
        text,
        "# Generated mapping provenance for Minecraft {}. Do not edit.\n\
     #\n\
     # Every line pins one resolved member. A change to the jar, the\n\
     # inventory, or the extractor changes this file, which is how drift is\n\
     # noticed before a build trusts a binding.\n\
     \n\
     schemaVersion = {}\n\
     minecraft = \"{}\"\n\
     mappingSource = \"{}\"\n\
     extractor = \"{EXTRACTOR}\"",
        adapter.minecraft,
        adapter.schema_version,
        adapter.minecraft,
        adapter.mapping_source.as_str()
    )
    .expect("writing to a String cannot fail");
    hash_pin(
        &mut text,
        "clientJarSha256",
        adapter.inputs.client_jar.sha256,
    );
    for (name, input) in [
        ("serverJarSha256", adapter.inputs.server_jar.as_ref()),
        (
            "clientMappingsSha256",
            adapter.inputs.client_mappings.as_ref(),
        ),
    ] {
        hash_pin(&mut text, name, input.and_then(|input| input.sha256));
    }
    for target in resolved {
        writeln!(
            text,
            "\n[target.\"{}\"]\nclass = \"{}\"\nmember = \"{}\"\ndescriptor = \"{}\"\nstatic = {}",
            target.id, target.mojang_class, target.member, target.descriptor, target.is_static
        )
        .expect("writing to a String cannot fail");
        // A linked name is written only where the mapping source made one, so a
        // lock under a source that translates nothing says each name once and a
        // reader can see at a glance which names moved.
        for (key, reviewed, linked) in [
            ("runtimeClass", &target.mojang_class, &target.runtime_class),
            ("runtimeMember", &target.member, &target.runtime_member),
            (
                "runtimeDescriptor",
                &target.descriptor,
                &target.runtime_descriptor,
            ),
        ] {
            if reviewed != linked {
                writeln!(text, "{key} = \"{linked}\"").expect("writing to a String cannot fail");
            }
        }
    }
    text
}

/// Renders the stub provenance lock.
pub(crate) fn stub_lock(adapter: &Adapter, stubs: &[Stub]) -> String {
    let mut text = String::with_capacity(stubs.len() * 256 + 1536);
    writeln!(
        text,
        "# Generated stub provenance for Minecraft {}. Do not edit.\n\
     #\n\
     # Every entry below is one compile-time stub the settings screen is built\n\
     # against, read out of the pinned jars: the client jar first, then the\n\
     # serialization library the game's own signatures name. Descriptors and\n\
     # access flags are the jar's own, which is the point: a hand-written stub\n\
     # transcribes a generic signature, and the JVM links on the erased one.\n\
     #\n\
     # What this file does not promise:\n\
     #\n\
     # - It is checked only where the pinned jars are. A machine without them\n\
     #   generates from this file, so on that machine the lock is the source and\n\
     #   cannot disagree with anything. Drift is caught where the jars exist.\n\
     # - It pins against jar hashes, so it is only as current as those pins. A\n\
     #   Minecraft build nobody has re-pinned against is unchecked.\n\
     # - It carries no hierarchy the stub set does not already hold. A supertype\n\
     #   outside the set is dropped, so a change to one is invisible here.\n\
     # - It covers the members `stubs.toml` seeds and nothing else. A member the\n\
     #   screen starts calling is unpinned until it is seeded.\n\
     #\n\
     # This is a different table from mappings.lock and stays one. That file\n\
     # pins call sites the agent rewrites; this one pins declarations the screen\n\
     # compiles against, and one table meaning both is how the next person\n\
     # misreads it.\n\
     \n\
     schemaVersion = {}\n\
     minecraft = \"{}\"\n\
     extractor = \"{EXTRACTOR}\"",
        adapter.minecraft, adapter.schema_version, adapter.minecraft
    )
    .expect("writing to a String cannot fail");
    hash_pin(
        &mut text,
        "clientJarSha256",
        adapter.inputs.client_jar.sha256,
    );
    hash_pin(
        &mut text,
        "dataFixerJarSha256",
        adapter
            .inputs
            .data_fixer_jar
            .as_ref()
            .and_then(|input| input.sha256),
    );

    stub_lock_entries(&mut text, stubs);
    text
}

fn stub_lock_entries(text: &mut String, stubs: &[Stub]) {
    for stub in stubs {
        writeln!(
            text,
            "\n[[stub]]\nclass = \"{}\"\nflags = {}",
            stub.class, stub.flags
        )
        .expect("writing to a String cannot fail");
        if let Some(parent) = &stub.extends {
            writeln!(text, "extends = \"{parent}\"").expect("writing to a String cannot fail");
        }
        if let Some(chained) = &stub.super_constructor {
            writeln!(text, "superConstructor = \"{chained}\"")
                .expect("writing to a String cannot fail");
        }
        if !stub.implements.is_empty() {
            let rendered = stub
                .implements
                .iter()
                .map(|name| format!("\"{name}\""))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(text, "implements = [{rendered}]").expect("writing to a String cannot fail");
        }
        for member in &stub.members {
            writeln!(
                text,
                "\n[[stub.member]]\nname = \"{}\"\ndescriptor = \"{}\"\nflags = {}\nmethod = {}",
                member.name, member.descriptor, member.flags, member.method
            )
            .expect("writing to a String cannot fail");
            if let Some(element) = &member.returns {
                writeln!(text, "returns = \"{element}\"").expect("writing to a String cannot fail");
            }
        }
    }
}

#[cfg(test)]
mod hash_tests {
    #[test]
    fn absent_hashes_are_not_rendered_as_empty_identities() {
        let mut text = String::new();
        super::hash_pin(&mut text, "clientJarSha256", None);
        assert_eq!(text, "");
        let hash = "ab".repeat(32);
        let digest = hash.parse().expect("a lowercase SHA-256 digest");
        super::hash_pin(&mut text, "clientJarSha256", Some(digest));
        assert_eq!(text, format!("clientJarSha256 = \"{hash}\"\n"));
    }
}
