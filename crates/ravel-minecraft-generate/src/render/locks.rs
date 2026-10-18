//! Renders mapping and stub provenance without changing inventory order.

use std::fmt::Write as _;

use crate::EXTRACTOR;
use crate::inputs::Adapter;
use crate::resolve::Resolved;
use crate::stubs::Stub;

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
     extractor = \"{EXTRACTOR}\"\n\
     clientJarSha256 = \"{}\"",
        adapter.minecraft,
        adapter.schema_version,
        adapter.minecraft,
        adapter.mapping_source.as_str(),
        adapter.inputs.client_jar.sha256.as_deref().unwrap_or("")
    )
    .expect("writing to a String cannot fail");
    if let Some(server) = &adapter.inputs.server_jar {
        writeln!(
            text,
            "serverJarSha256 = \"{}\"",
            server.sha256.as_deref().unwrap_or("")
        )
        .expect("writing to a String cannot fail");
    }
    // A source that reads no mapping has none to pin, so the key exists only
    // where one decided the linked names below.
    if let Some(mappings) = &adapter.inputs.client_mappings {
        writeln!(
            text,
            "clientMappingsSha256 = \"{}\"",
            mappings.sha256.as_deref().unwrap_or("")
        )
        .expect("writing to a String cannot fail");
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
     extractor = \"{EXTRACTOR}\"\n\
     clientJarSha256 = \"{}\"",
        adapter.minecraft,
        adapter.schema_version,
        adapter.minecraft,
        adapter.inputs.client_jar.sha256.as_deref().unwrap_or("")
    )
    .expect("writing to a String cannot fail");
    if let Some(library) = &adapter.inputs.data_fixer_jar {
        writeln!(
            text,
            "dataFixerJarSha256 = \"{}\"",
            library.sha256.as_deref().unwrap_or("")
        )
        .expect("writing to a String cannot fail");
    }

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
