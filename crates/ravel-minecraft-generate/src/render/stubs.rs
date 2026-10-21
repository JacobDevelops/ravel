//! Java source rendering for compile-time stubs.

use std::fmt::Write as _;

use crate::classfile::{ACC_FINAL, ACC_PROTECTED, ACC_PUBLIC, ACC_STATIC};
use crate::classfile::{java_type, signature, source_name, split};
use crate::error::GenerateError;
use crate::inputs::Adapter;
use crate::mapping::Mapping;
use crate::stubs::{self, Member, Stub};

/// Renders a stub under the names the JVM links.
pub(crate) fn stub_java(
    adapter: &Adapter,
    stub: &Stub,
    mapping: &Mapping,
) -> Result<(String, String), GenerateError> {
    let unsupported =
        |detail: String| GenerateError::Unsupported(format!("{}: {detail}", stub.class));
    let linked = mapping.runtime_class_or_self(&stub.class);
    let (package, simple) = split(linked);

    let mut text = String::with_capacity(stub.members.len() * 192 + 1024);
    writeln!(
        text,
        "// Generated from the pinned Minecraft {} class files. Do not edit.\n\
     //\n\
     // A compile-time stub, not Minecraft. It is compiled against and then\n\
     // discarded: `build-screen.sh` refuses to package one, because a stub in\n\
     // the screen jar would shadow the game's own class on the loader they\n\
     // share. See minecraft/versions/{}/stubs.lock.",
        adapter.minecraft, adapter.minecraft
    )
    .expect("writing to a String cannot fail");
    // Classes in the unnamed package must omit the package declaration.
    match &package {
        Some(package) => {
            writeln!(text, "package {package};\n").expect("writing to a String cannot fail");
        }
        None => text.push('\n'),
    }

    writeln!(text, "{} {{", declaration(stub, mapping, simple))
        .expect("writing to a String cannot fail");

    for member in &stub.members {
        let modifiers = modifiers(member.flags, stub.is_interface(), member.method);
        let (name, descriptor) =
            mapping.runtime_member_or_self(&stub.class, &member.name, &member.descriptor)?;
        let rendered = if member.method {
            stub_method(
                stub,
                member,
                &modifiers,
                &name,
                &descriptor,
                simple,
                mapping,
            )
        } else {
            stub_field(stub, member, &modifiers, &name, &descriptor, mapping)
        };
        writeln!(text, "{}", rendered.map_err(unsupported)?)
            .expect("writing to a String cannot fail");
    }

    writeln!(text, "}}").expect("writing to a String cannot fail");
    Ok((format!("{linked}.java"), text))
}

fn declaration(stub: &Stub, mapping: &Mapping, simple: &str) -> String {
    let mut declaration = String::new();
    if stub.flags & ACC_PUBLIC != 0 {
        declaration.push_str("public ");
    }
    if stub.is_interface() {
        declaration.push_str("interface ");
    } else {
        if stub.is_abstract() {
            declaration.push_str("abstract ");
        }
        if stub.is_final() {
            declaration.push_str("final ");
        }
        declaration.push_str("class ");
    }
    declaration.push_str(simple);
    if let Some(parent) = &stub.extends {
        write!(
            declaration,
            " extends {}",
            source_name(mapping.runtime_class_or_self(parent))
        )
        .expect("writing to a String cannot fail");
    }
    if !stub.implements.is_empty() {
        declaration.push_str(if stub.is_interface() && stub.extends.is_none() {
            " extends "
        } else if stub.is_interface() {
            ", "
        } else {
            " implements "
        });
        declaration.push_str(
            &stub
                .implements
                .iter()
                .map(|name| source_name(mapping.runtime_class_or_self(name)))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    declaration
}

fn stub_method(
    stub: &Stub,
    member: &Member,
    modifiers: &str,
    name: &str,
    descriptor: &str,
    simple: &str,
    mapping: &Mapping,
) -> Result<String, String> {
    let (parameters, erased) = signature(descriptor)?;
    // Preserve the recorded element type as well as the erased JVM return.
    let returns = match &member.returns {
        Some(recorded) => format!("{erased}<{}>", argument(recorded, mapping)?),
        None => erased,
    };
    let declared = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| format!("{parameter} argument{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    if member.name == "<init>" {
        let chained = chained_super(stub, mapping)?;
        return Ok(format!(
            "\n    {modifiers}{simple}({declared}) {{\n{chained}    }}"
        ));
    }
    if returns == "void" {
        return Ok(format!(
            "\n    {modifiers}void {name}({declared}) {{\n    }}"
        ));
    }
    // Accidental execution must fail rather than fabricate a return value.
    Ok(format!(
        "\n    {modifiers}{returns} {name}({declared}) {{\n        \
         throw new UnsupportedOperationException(\"stub\");\n    }}"
    ))
}

fn stub_field(
    stub: &Stub,
    member: &Member,
    modifiers: &str,
    name: &str,
    descriptor: &str,
    mapping: &Mapping,
) -> Result<String, String> {
    if stub.is_interface() {
        return Err(format!(
            "{} is a field on an interface, which is implicitly final and would need a value no \
             stub can invent",
            member.name
        ));
    }
    let (kind, rest) = java_type(descriptor)?;
    if !rest.is_empty() {
        return Err(format!(
            "{} declares {} which is not one type",
            member.name, member.descriptor
        ));
    }
    let kind = match &member.returns {
        Some(recorded) => format!("{kind}<{}>", argument(recorded, mapping)?),
        None => kind,
    };
    Ok(format!("\n    {modifiers}{kind} {name};"))
}

fn argument(recorded: &str, mapping: &Mapping) -> Result<String, String> {
    let (bound, class) = stubs::argument(recorded)
        .ok_or_else(|| format!("{recorded} is not a type argument over one class"))?;
    Ok(format!(
        "{}{}",
        bound.java(),
        source_name(mapping.runtime_class_or_self(class))
    ))
}

// Explicit super calls support superclasses without a no-argument constructor.
fn chained_super(stub: &Stub, mapping: &Mapping) -> Result<String, String> {
    let Some(recorded) = stub.super_constructor.as_ref() else {
        return Ok(String::new());
    };
    let (_, linked) = mapping
        .runtime_member_or_self(&stub.class, "<init>", recorded)
        .map_err(|error| error.to_string())?;
    let (parameters, _) = signature(&linked)?;
    Ok(format!("        super({});\n", placeholders(&parameters)))
}

// Cast nulls disambiguate overloaded superclass constructors.
fn placeholders(parameters: &[String]) -> String {
    parameters
        .iter()
        .map(|parameter| {
            crate::java_type::JavaType::primitive_from_keyword(parameter).map_or_else(
                || format!("({parameter}) null"),
                |kind| kind.zero_literal().to_owned(),
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

// Stub methods have bodies, so never emit abstract.
fn modifiers(flags: u16, interface: bool, method: bool) -> String {
    let mut text = String::new();
    if flags & ACC_PUBLIC != 0 && !interface {
        text.push_str("public ");
    } else if flags & ACC_PROTECTED != 0 {
        text.push_str("protected ");
    }
    if flags & ACC_STATIC != 0 {
        text.push_str("static ");
    } else if interface && method {
        text.push_str("default ");
    }
    if flags & ACC_FINAL != 0 && method {
        text.push_str("final ");
    }
    text
}
