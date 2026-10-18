//! Renders JNI exports from validated hook shapes; no Java source is emitted here.

use std::fmt::Write as _;

use crate::HOOK_CLASS;

use super::shape::{JavaType, Shape};

/// Renders the JNI export stubs the bootstrap includes.
pub(crate) fn exports(shapes: &[Shape]) -> String {
    let mut text = String::with_capacity(shapes.len() * 512 + 768);
    text.push_str(
        "// Generated JNI exports for the pinned hook surface. Do not edit.\n\
         //\n\
         // One export per hook shape: a single dispatcher\n\
         // taking an argument array would allocate on every crossing, and the\n\
         // measured crossing allocates nothing. Every declaration is emitted by\n\
         // the generator, which is what lets this crate keep\n\
         // `#![forbid(unsafe_code)]` verbatim.\n\n",
    );

    for shape in shapes {
        let names = (0..shape.parameters.len())
            .map(|index| format!("argument{index}"))
            .collect::<Vec<_>>();
        let carries_text = shape.parameters.contains(&JavaType::Text);

        writeln!(
            text,
            "/// {}\n\
         #[::jni::jni_mangle(\"{HOOK_CLASS}\", \"{}\")]\n\
         pub fn {}<'local>(\n\
         \x20   env: ::jni::EnvUnowned<'local>,\n\
         \x20   _class: ::jni::objects::JClass<'local>,",
            shape.capability, shape.java_method, shape.rust_function
        )
        .expect("writing to a String cannot fail");
        for (name, parameter) in names.iter().zip(&shape.parameters) {
            writeln!(text, "    {name}: {},", parameter.rust())
                .expect("writing to a String cannot fail");
        }

        let values = if names.is_empty() {
            "::std::vec::Vec::new()".to_owned()
        } else {
            let rendered = names
                .iter()
                .zip(&shape.parameters)
                .map(|(name, parameter)| parameter.value(name))
                .collect::<Vec<_>>()
                .join(",\n            ");
            format!("::std::vec![\n            {rendered},\n        ]")
        };

        writeln!(
            text,
            ") {{\n\
         \x20   crate::guard::enter(\n\
         \x20       env,\n\
         \x20       &::ravel_minecraft_adapter::pinned::{},\n\
         \x20       |{}| ::std::result::Result::Ok({values}),\n\
         \x20   );\n\
         }}\n",
            shape.constant,
            if carries_text { "env" } else { "_env" }
        )
        .expect("writing to a String cannot fail");
    }
    text
}
