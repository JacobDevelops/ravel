//! Validates hook descriptors and derives the shared Java/JNI hook shape.

use crate::classfile::method_types;
use crate::error::GenerateError;
use crate::inputs::Inventory;
use crate::java_type::JavaType;
use crate::resolve::Resolved;

/// One method hook, with every name the generated artefacts need.
#[derive(Clone, Debug)]
pub(crate) struct Shape {
    pub(super) capability: String,
    pub(super) constant: String,
    pub(super) rust_function: String,
    pub(super) java_method: String,
    pub(super) parameters: Vec<JavaType>,
    pub(super) runtime_class: String,
    pub(super) runtime_member: String,
    pub(super) runtime_descriptor: String,
    pub(super) is_static: bool,
}

impl Shape {
    /// The descriptor of the generated hook method.
    ///
    /// Observation returns nothing and does not decide whether the target runs.
    /// The injected call site needs no verdict branch or discarded value.
    pub(super) fn hook_descriptor(&self) -> String {
        let mut text = String::from("(");
        for parameter in &self.parameters {
            text.push_str(parameter.descriptor());
        }
        text.push_str(")V");
        text
    }
}

/// Derives every method hook's generated names, refusing anything unrepresentable.
pub(crate) fn shapes(
    inventory: &Inventory,
    resolved: &[Resolved],
) -> Result<Vec<Shape>, GenerateError> {
    let mut shapes = Vec::new();
    for (hook, target) in inventory.hooks.iter().zip(resolved) {
        if !hook.kind.is_method() {
            continue;
        }
        shapes.push(Shape {
            capability: hook.capability.clone(),
            constant: constant_name(&hook.id),
            rust_function: rust_name(&hook.id),
            java_method: java_name(&hook.id),
            // The reviewed descriptor, because the only thing a parameter keeps
            // of its class is the string `JavaHandle::new` declares, and that
            // one is read by a mod rather than linked by the JVM.
            parameters: parameters(&target.descriptor)
                .map_err(|detail| GenerateError::Unsupported(format!("{}: {detail}", hook.id)))?,
            runtime_class: target.runtime_class.clone(),
            runtime_member: target.runtime_member.clone(),
            runtime_descriptor: target.runtime_descriptor.clone(),
            is_static: target.is_static,
        });
    }

    // The generated Java class never overloads, which is what lets the exported
    // symbols use JNI's short mangled form. Two identifiers that differ only in
    // their separators would break that, so they are refused here rather than at
    // the first hook invocation.
    let mut names = shapes
        .iter()
        .map(|shape| shape.java_method.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    let total = names.len();
    names.dedup();
    if names.len() != total {
        return Err(GenerateError::Unsupported(
            "two reviewed hooks generate the same Java method name".to_owned(),
        ));
    }
    Ok(shapes)
}

/// Reads the parameter types out of a JVM method descriptor.
pub(super) fn parameters(descriptor: &str) -> Result<Vec<JavaType>, String> {
    let (parameters, _) = method_types(descriptor)?;
    Ok(parameters
        .into_iter()
        .map(|kind| {
            if kind.dimensions != 0 {
                return if kind.descriptor == "[B" {
                    JavaType::Bytes
                } else {
                    JavaType::Handle(kind.descriptor.to_owned())
                };
            }
            if kind.descriptor.starts_with('L') {
                return if kind.name == "java/lang/String" {
                    JavaType::Text
                } else {
                    JavaType::Handle(kind.name.to_owned())
                };
            }
            JavaType::primitive_from_descriptor(kind.descriptor)
                .expect("a validated non-reference parameter is a primitive")
        })
        .collect())
}

/// Turns a hook identifier into a Rust constant name.
pub(super) fn constant_name(id: &str) -> String {
    id.replace(['.', '-'], "_").to_uppercase()
}

/// Turns a hook identifier into a Rust function name.
pub(super) fn rust_name(id: &str) -> String {
    id.replace(['.', '-'], "_").to_lowercase()
}

/// Turns a hook identifier into a Java method name.
pub(super) fn java_name(id: &str) -> String {
    let mut name = String::with_capacity(id.len());
    for (index, segment) in id
        .split(['.', '-', '_'])
        .filter(|s| !s.is_empty())
        .enumerate()
    {
        let mut characters = segment.chars();
        let Some(first) = characters.next() else {
            continue;
        };
        if index == 0 {
            name.extend(first.to_lowercase());
        } else {
            name.extend(first.to_uppercase());
        }
        name.push_str(characters.as_str());
    }
    name
}
