//! Artifact-specific rendering from resolved inputs and validated hook shapes.
//!
//! Shape validation is shared by Java hooks and JNI exports; provenance locks,
//! Rust bindings, and reflective Java names each own their output format.

pub(super) mod bindings;
pub(super) mod exports;
pub(super) mod hooks;
pub(super) mod locks;
pub(super) mod names;
pub(super) mod shape;
pub(super) mod stubs;

#[cfg(test)]
mod tests {
    use super::super::classfile::{alias, java_type, name_space_is_split, signature};
    use super::super::inputs::{
        Adapter, Hook, Input, Inputs, Inventory, Kind, MappingSource, PackFormat, Reflected,
        Reflection, Spelling,
    };
    use super::super::resolve::Resolved;
    use super::super::stubs::{Member as StubMember, Stub};
    use super::bindings::bindings;
    use super::exports::exports;
    use super::hooks::hook_java;
    use super::locks::stub_lock;
    use super::names::game_names;
    use super::shape::{JavaType, constant_name, java_name, parameters, rust_name, shapes};
    use super::stubs::stub_java;
    use crate::mapping::Mapping;

    #[test]
    fn mapping_locks_preserve_order_and_record_only_translated_names() {
        let mut adapter = adapter();
        adapter.mapping_source = MappingSource::OfficialProguardMappings;
        let mut mappings = adapter.inputs.client_jar.clone();
        mappings.sha256 = Some("reviewed-mappings".to_owned());
        adapter.inputs.client_mappings = Some(mappings);
        let (_, mut resolved) = reviewed(&[CANDIDATES[1], CANDIDATES[0]]);
        resolved[0].runtime_member = "b".to_owned();

        let lock = super::locks::lock(&adapter, &resolved);

        assert!(lock.contains("clientMappingsSha256 = \"reviewed-mappings\""));
        let (translated, unchanged) = lock
            .split_once("[target.\"registry.register\"]")
            .expect("second target");
        assert!(translated.contains("[target.\"server.tick\"]"));
        assert!(translated.contains("runtimeMember = \"b\""));
        assert!(!translated.contains("runtimeClass"));
        assert!(!translated.contains("runtimeDescriptor"));
        assert!(!unchanged.contains("runtime"));
    }

    #[test]
    fn bindings_keep_pack_minors_spelling_and_reviewed_target_order() {
        let mut adapter = adapter();
        adapter.mapping_source = MappingSource::OfficialProguardMappings;
        adapter.pack_format.resource_minor = 2;
        adapter.pack_format.data_minor = 3;
        adapter.pack_format.spelling = Spelling::MinMaxFormat;
        let (mut inventory, mut resolved) = reviewed(&[CANDIDATES[1], CANDIDATES[0]]);
        inventory.hooks[0].kind = Kind::StaticMethod;
        resolved[0].is_static = true;
        resolved[0].runtime_class = "a".to_owned();
        resolved[0].runtime_member = "b".to_owned();
        resolved[0].runtime_descriptor = "(Lc;)Ld;".to_owned();

        let rust = bindings(&adapter, &inventory, &resolved);

        assert!(rust.contains("RESOURCE_PACK_FORMAT: PackFormat = PackFormat::new(7, 2);"));
        assert!(rust.contains("DATA_PACK_FORMAT: PackFormat = PackFormat::new(11, 3);"));
        assert!(rust.contains("PackFormatSpelling::MinMaxFormat;"));
        assert!(rust.contains("\"a\",\n    \"b\",\n    \"(Lc;)Ld;\",\n    HookKind::StaticMethod"));
        assert!(
            rust.contains("TARGETS: [HookTarget; 2] = [\n    SERVER_TICK,\n    REGISTRY_REGISTER,")
        );
    }

    #[test]
    fn exports_copy_text_borrow_bytes_and_allocate_no_empty_argument_vector() {
        let (inventory, resolved) = reviewed(&[
            ("payload.copy", "a.B", "copy", "(Ljava/lang/String;[B)V"),
            ("payload.empty", "a.B", "empty", "()V"),
        ]);
        let shapes = shapes(&inventory, &resolved).expect("valid shapes");
        let rust = exports(&shapes);

        assert!(rust.contains("argument0: ::jni::objects::JString<'local>"));
        assert!(rust.contains("argument1: ::jni::objects::JByteArray<'local>"));
        assert!(rust.contains("|env| ::std::result::Result::Ok("));
        assert!(rust.contains("HookValue::Text(crate::guard::text(env, &argument0)?)"));
        assert!(rust.contains("JavaHandle::new(\"[B\", &argument1)"));
        assert!(rust.contains("|_env| ::std::result::Result::Ok(::std::vec::Vec::new())"));
        let java = hook_java(&adapter(), &shapes);
        assert!(java.contains(
            "public static native void payloadCopy(String argument0, byte[] argument1);"
        ));
        assert!(java.contains("public static native void payloadEmpty();"));
    }

    #[test]
    fn member_names_skip_counted_lookups_and_refuse_colliding_constants() {
        use crate::inputs::ReflectedMember;

        let mut surface = Reflection {
            schema_version: 1,
            minecraft: "0.0.0-test".to_owned(),
            classes: vec![Reflected {
                name: "SCREEN".to_owned(),
                class: "a.B".to_owned(),
                runtime: "a/B".to_owned(),
                members: vec![
                    ReflectedMember {
                        name: "<init>".to_owned(),
                        lookup: crate::inputs::Lookup::Descriptor("()V".to_owned()),
                    },
                    ReflectedMember {
                        name: "drawRow".to_owned(),
                        lookup: crate::inputs::Lookup::Descriptor("(I)V".to_owned()),
                    },
                    ReflectedMember {
                        name: "counted".to_owned(),
                        lookup: crate::inputs::Lookup::Parameters(1),
                    },
                ],
            }],
        };
        let java = super::names::game_members(&adapter(), &surface, &Mapping::Identity)
            .expect("distinct constants");
        assert!(java.contains("SCREEN_NEW = \"<init>:()V\";"));
        assert!(java.contains("SCREEN_DRAW_ROW = \"drawRow:(I)V\";"));
        assert!(!java.contains("SCREEN_COUNTED"));
        surface.classes[0].members.push(ReflectedMember {
            name: "drawRow".to_owned(),
            lookup: crate::inputs::Lookup::Descriptor("(II)V".to_owned()),
        });
        let error = super::names::game_members(&adapter(), &surface, &Mapping::Identity)
            .expect_err("overloads cannot share a constant");
        assert!(
            error
                .to_string()
                .contains("SCREEN_DRAW_ROW would be written down twice")
        );
    }

    #[test]
    fn menu_rows_distinguish_derived_placement_from_negative_offsets() {
        let (mut inventory, _) =
            reviewed(&[(crate::inputs::TITLE_MENU_SEAM, "a.B", "row", "(I)I")]);
        let derived = super::names::menu_layout(&adapter(), &inventory);
        assert!(derived.contains("RAVEL_ROW = DERIVED;"));
        inventory.hooks[0].descriptor = "()V".to_owned();
        inventory.hooks[0].title_menu_row = Some(-24);
        let explicit = super::names::menu_layout(&adapter(), &inventory);
        assert!(explicit.contains("RAVEL_ROW = -24;"));
        assert!(explicit.contains("DERIVED = Integer.MIN_VALUE;"));
    }

    #[test]
    fn mapping_provenance_pins_a_declared_server_without_changing_modern_locks() {
        let mut adapter = adapter();
        let modern = super::locks::lock(&adapter, &[]);
        assert!(!modern.contains("serverJarSha256"));
        let mut server = adapter.inputs.client_jar.clone();
        server.sha256 = Some("reviewed-server-hash".to_owned());
        adapter.inputs.server_jar = Some(server);
        let legacy = super::locks::lock(&adapter, &[]);
        let pin = "serverJarSha256 = \"reviewed-server-hash\"\n";
        assert!(legacy.contains(pin));
        assert_eq!(legacy.replace(pin, ""), modern);
    }

    /// Adapter inputs standing in for the tracked ones.
    ///
    /// Rendering needs a version string and nothing else from them, and building
    /// one here is what keeps a candidate hook out of `hooks.toml`: growing the
    /// reviewed inventory is a separate, deliberate step that re-pins a hash and
    /// regenerates the lock, and proving the generator can carry a shape is not.
    fn adapter() -> Adapter {
        let unpinned = || Input {
            path: String::new(),
            sha256: None,
            bytes: None,
            upstream: None,
        };
        Adapter {
            schema_version: 1,
            minecraft: "0.0.0-test".to_owned(),
            eligibility: crate::inputs::Eligibility::Live,
            mapping_source: MappingSource::UnobfuscatedClassFiles,
            mapping_namespaces: None,
            capabilities: Vec::new(),
            java_release: 25,
            source_java_release: None,
            pack_format: PackFormat {
                resource: 7,
                resource_minor: 0,
                data: 11,
                data_minor: 0,
                spelling: Spelling::PackFormat,
            },
            inputs: Inputs {
                client_jar: unpinned(),
                server_jar: None,
                client_mappings: None,
                mapping_io_jar: None,
                data_fixer_jar: Some(unpinned()),
                version_manifest: unpinned(),
                hook_inventory: unpinned(),
                stub_seeds: unpinned(),
                reflective_surface: unpinned(),
                mappings_lock: unpinned(),
                stubs_lock: unpinned(),
            },
        }
    }

    /// The alias moves a class only where the two name spaces meet, and moves
    /// it to the one the other half is already in.
    #[test]
    fn a_split_name_space_writes_every_class_where_the_unnamed_half_already_is() {
        assert!(name_space_is_split(["a", "com/example/Held"]));
        assert!(!name_space_is_split(["a", "b"]));
        assert!(!name_space_is_split([
            "com/example/Held",
            "com/example/Kept"
        ]));

        assert_eq!(alias("com/example/Held", true), "Held");
        assert_eq!(alias("a", true), "a");
        assert_eq!(alias("com/example/Held", false), "com/example/Held");
    }

    /// What the agent resolves is the name the version links, whatever name the
    /// stand-in beside it was written under.
    ///
    /// The two are one string under every version the harness can run, and a
    /// generator that aliased this one would make the harness agree with itself
    /// and with nothing the game loads.
    #[test]
    fn the_names_the_adapters_resolve_are_the_linked_ones_under_a_split_name_space() {
        let surface = Reflection {
            schema_version: 1,
            minecraft: String::from("0.0.0"),
            classes: vec![
                Reflected {
                    name: String::from("SERVER"),
                    class: String::from("com.example.server.Held"),
                    runtime: String::from("com/example/server/Held"),
                    members: Vec::new(),
                },
                Reflected {
                    name: String::from("SURFACE"),
                    class: String::from("com.example.gui.Panel"),
                    runtime: String::from("a"),
                    members: Vec::new(),
                },
            ],
        };

        let java = game_names(&adapter(), &surface);

        assert!(
            java.contains("SERVER = \"com/example/server/Held\";"),
            "{java}"
        );
        assert!(java.contains("SURFACE = \"a\";"), "{java}");
    }

    /// An inventory and its resolution, from entries written the same way the
    /// tracked ones are.
    fn reviewed(entries: &[(&str, &str, &str, &str)]) -> (Inventory, Vec<Resolved>) {
        let hooks = entries
            .iter()
            .map(|(id, class, member, descriptor)| Hook {
                id: (*id).to_owned(),
                class: (*class).to_owned(),
                member: (*member).to_owned(),
                descriptor: (*descriptor).to_owned(),
                kind: Kind::InstanceMethod,
                capability: format!("A candidate {id} hook."),
                side: crate::inputs::HookSide::Shared,
                refuses: false,
                title_menu_row: None,
            })
            .collect::<Vec<_>>();
        let resolved = hooks
            .iter()
            .map(|hook| Resolved {
                id: hook.id.clone(),
                runtime_class: hook.class.replace('.', "/"),
                mojang_class: hook.class.replace('.', "/"),
                runtime_member: hook.member.clone(),
                member: hook.member.clone(),
                runtime_descriptor: hook.descriptor.clone(),
                descriptor: hook.descriptor.clone(),
                is_static: false,
            })
            .collect();
        (
            Inventory {
                schema_version: 1,
                minecraft: "0.0.0-test".to_owned(),
                hooks,
                non_goals: Vec::new(),
            },
            resolved,
        )
    }

    #[test]
    fn a_primitive_descriptor_reads_back_as_the_arguments_it_declares() {
        assert_eq!(parameters("(I)Z").expect("valid"), vec![JavaType::Int]);
        assert_eq!(
            parameters("(DD)D").expect("valid"),
            vec![JavaType::Double, JavaType::Double]
        );
        assert_eq!(parameters("()V").expect("valid"), Vec::new());
        assert_eq!(
            parameters("(ZBCSIJFD)V").expect("valid"),
            vec![
                JavaType::Boolean,
                JavaType::Byte,
                JavaType::Char,
                JavaType::Short,
                JavaType::Int,
                JavaType::Long,
                JavaType::Float,
                JavaType::Double,
            ]
        );
    }

    #[test]
    fn a_string_is_copied_and_every_other_reference_becomes_a_handle() {
        assert_eq!(
            parameters("(Ljava/lang/String;Z)Ljava/lang/String;").expect("valid"),
            vec![JavaType::Text, JavaType::Boolean]
        );
        assert_eq!(
            parameters("(Lcom/example/resources/ResourceKey;Ljava/lang/Object;)V")
                .expect("a reference to an object the game owns crosses as a handle"),
            vec![
                JavaType::Handle("com/example/resources/ResourceKey".to_owned()),
                JavaType::Handle("java/lang/Object".to_owned()),
            ]
        );
    }

    #[test]
    fn an_array_is_an_object_and_crosses_as_a_handle_naming_its_own_descriptor() {
        assert_eq!(
            parameters("([J[[ILjava/lang/String;)V").expect("valid"),
            vec![
                JavaType::Handle("[J".to_owned()),
                JavaType::Handle("[[I".to_owned()),
                JavaType::Text,
            ]
        );
        assert_eq!(
            parameters("([Ljava/lang/String;)V").expect("valid"),
            vec![JavaType::Handle("[Ljava/lang/String;".to_owned())],
            "an array of strings is an array, and an array is never copied"
        );
    }

    #[test]
    fn a_byte_array_crosses_as_bytes_and_no_other_array_does() {
        assert_eq!(
            parameters("([BI)V").expect("valid"),
            vec![JavaType::Bytes, JavaType::Int],
            "the saved-data aggregate is a byte payload rather than a borrowed object"
        );
        assert_eq!(
            parameters("([[B[Ljava/lang/Byte;)V").expect("valid"),
            vec![
                JavaType::Handle("[[B".to_owned()),
                JavaType::Handle("[Ljava/lang/Byte;".to_owned()),
            ],
            "only `[B` is the payload; an array of them is an array like any other"
        );
        assert_eq!(JavaType::Bytes.descriptor(), "[B");
        assert_eq!(JavaType::Bytes.java(), "byte[]");
        assert_eq!(JavaType::Bytes.rust(), "::jni::objects::JByteArray<'local>");
    }

    #[test]
    fn a_descriptor_that_is_not_one_is_refused_and_says_what_about_it_is_wrong() {
        assert!(
            parameters("(Lcom/example/core/Registry)V")
                .expect_err("an unterminated class name must be refused")
                .contains("never terminated")
        );
        assert!(
            parameters("([)V")
                .expect_err("an array of nothing must be refused")
                .contains("not a type descriptor")
        );
        assert!(
            parameters("(L;)V")
                .expect_err("a class descriptor naming no class must be refused")
                .contains("names no class")
        );
        assert!(
            parameters("(Q)V")
                .expect_err("an unknown descriptor must be refused")
                .contains("not a type descriptor")
        );
        assert!(
            parameters("(V)V")
                .expect_err("void is not a parameter")
                .contains("not a type descriptor")
        );
        assert!(
            parameters("I)V")
                .expect_err("this is not a method descriptor")
                .contains("not a method descriptor")
        );
    }

    #[test]
    fn every_descriptor_consumer_rejects_invalid_method_returns_and_void_arrays() {
        for descriptor in ["(I)", "(I)VZ", "(I)Q", "(I)[V", "([V)V", "(I", "()"] {
            assert!(parameters(descriptor).is_err(), "{descriptor}");
            assert!(signature(descriptor).is_err(), "{descriptor}");
            assert!(
                crate::descriptor_parameters(descriptor).is_err(),
                "{descriptor}"
            );
            assert!(
                crate::descriptor_handle_count(descriptor).is_err(),
                "{descriptor}"
            );
        }
    }

    #[test]
    fn shared_descriptor_inventory_matches_generated_handle_classification() {
        let descriptor = "(ZBCSIJFDLjava/lang/String;Ljava/lang/Object;[B[[B[Ljava/lang/String;)V";
        let kinds = parameters(descriptor).expect("hook parameters");
        let handles = kinds
            .iter()
            .filter(|kind| matches!(kind, JavaType::Handle(_)))
            .count();
        assert_eq!(handles, 3);
        assert_eq!(
            crate::descriptor_handle_count(descriptor).expect("inventory count"),
            handles
        );
        assert_eq!(
            crate::descriptor_parameters("([B[[BLjava/lang/String;)V").expect("exact tokens"),
            ["[B", "[[B", "Ljava/lang/String;"]
        );
        assert_eq!(
            signature("([B[[BLjava/lang/String;)V").expect("source spelling"),
            (
                vec![
                    "byte[]".to_owned(),
                    "byte[][]".to_owned(),
                    "java.lang.String".to_owned()
                ],
                "void".to_owned()
            )
        );
    }

    #[test]
    fn every_admitted_type_round_trips_through_the_hook_signature_it_renders() {
        for parameter in [
            JavaType::Boolean,
            JavaType::Byte,
            JavaType::Char,
            JavaType::Short,
            JavaType::Int,
            JavaType::Long,
            JavaType::Float,
            JavaType::Double,
            JavaType::Text,
            JavaType::Bytes,
            JavaType::Handle("java/lang/Object".to_owned()),
        ] {
            let descriptor = format!("({}){}", parameter.descriptor(), parameter.descriptor());

            assert_eq!(
                parameters(&descriptor).expect("valid"),
                vec![parameter.clone()]
            );
            assert_ne!(parameter.java(), "");
            assert!(parameter.rust().starts_with("::jni::"));
            assert!(parameter.value("x").contains("HookValue::"));
        }
    }

    #[test]
    fn a_handle_keeps_its_declared_class_in_rust_and_loses_it_in_the_hook_signature() {
        let registry = JavaType::Handle("com/example/core/MappedRegistry".to_owned());

        assert_eq!(registry.descriptor(), "Ljava/lang/Object;");
        assert_eq!(registry.java(), "Object");
        assert_eq!(registry.rust(), "::jni::objects::JObject<'local>");
        assert_eq!(
            registry.value("argument0"),
            "::ravel_minecraft::HookValue::Handle(::ravel_minecraft::JavaHandle::new(\
             \"com/example/core/MappedRegistry\", &argument0))"
        );
    }

    /// The real M3 targets this enables, plus one array shape the resolved set
    /// has no parameter for yet.
    ///
    /// `MappedRegistry.register` is the entry that decided ADG-4's amendment: a
    /// registry hook that cannot name what is being registered is not a hook.
    const CANDIDATES: [(&str, &str, &str, &str); 3] = [
        (
            "registry.register",
            "com.example.core.MappedRegistry",
            "register",
            "(Lcom/example/resources/ResourceKey;Ljava/lang/Object;\
             Lcom/example/core/RegistrationInfo;)Lcom/example/core/Holder$Reference;",
        ),
        (
            "server.tick",
            "com.example.server.MinecraftServer",
            "tickServer",
            "(Ljava/util/function/BooleanSupplier;)V",
        ),
        (
            "server.tick-times",
            "com.example.server.MinecraftServer",
            "logTickTimes",
            "([JZ)V",
        ),
    ];

    #[test]
    fn a_candidate_carrying_objects_renders_a_borrowed_handle_per_reference() {
        let (inventory, resolved) = reviewed(&CANDIDATES);

        let shapes = shapes(&inventory, &resolved).expect("every candidate should render");
        let rust = exports(&shapes);

        assert_eq!(shapes.len(), 3);
        assert!(
            rust.contains("argument0: ::jni::objects::JObject<'local>,"),
            "{rust}"
        );
        assert!(
            rust.contains(
                "::ravel_minecraft::JavaHandle::new(\"com/example/resources/ResourceKey\", \
                 &argument0)"
            ),
            "{rust}"
        );
        assert!(
            rust.contains("::ravel_minecraft::JavaHandle::new(\"[J\", &argument0)"),
            "an array is an object and names its own descriptor: {rust}"
        );
        assert!(
            rust.contains("|_env|"),
            "no handle needs the environment, so no export asks for it: {rust}"
        );
    }

    /// The class a handle declares is what a mod reads back off it, so it comes
    /// from the reviewed descriptor even where the JVM links an obfuscated one.
    #[test]
    fn a_handle_declares_the_reviewed_class_where_the_mapping_obfuscates_it() {
        let (inventory, mut resolved) = reviewed(&CANDIDATES[..1]);
        resolved[0].runtime_class = "ezb".to_owned();
        resolved[0].runtime_member = "a".to_owned();
        resolved[0].runtime_descriptor = "(Lfhk;Ljava/lang/Object;Lezc;)Lezd;".to_owned();

        let shapes = shapes(&inventory, &resolved).expect("the candidate should render");
        let rust = exports(&shapes);
        let table = bindings(&adapter(), &inventory, &resolved);

        assert!(
            rust.contains(
                "::ravel_minecraft::JavaHandle::new(\"com/example/resources/ResourceKey\", \
                 &argument0)"
            ),
            "{rust}"
        );
        assert!(
            !rust.contains("JavaHandle::new(\"fhk\""),
            "no linked name reaches an observer: {rust}"
        );
        assert!(
            table.contains("(Lfhk;Ljava/lang/Object;Lezc;)Lezd;"),
            "the target table still links the runtime descriptor: {table}"
        );
    }

    #[test]
    fn the_generated_java_takes_object_and_names_no_game_type() {
        let (inventory, resolved) = reviewed(&CANDIDATES);

        let shapes = shapes(&inventory, &resolved).expect("every candidate should render");
        let java = hook_java(&adapter(), &shapes);

        assert!(
            java.contains(
                "public static native void registryRegister(Object argument0, \
                 Object argument1, Object argument2);"
            ),
            "{java}"
        );
        assert!(
            java.contains("\"(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)V\""),
            "the hook descriptor the agent injects erases every class: {java}"
        );
        assert!(
            java.contains("\"(Ljava/lang/Object;Z)V\""),
            "an array is one parameter and erases to Object beside the boolean: {java}"
        );
        for declaration in java.lines().filter(|line| line.contains("static native")) {
            assert!(
                !declaration.contains("minecraft"),
                "Hook.java compiles without the client jar, so no declaration names a \
                 game type: {declaration}"
            );
        }
        assert!(
            java.contains(
                "\"(Lcom/example/resources/ResourceKey;Ljava/lang/Object;\
                 Lcom/example/core/RegistrationInfo;)Lcom/example/core/Holder$Reference;\""
            ),
            "the target keeps its own descriptor, which is what the agent matches on: {java}"
        );
    }

    /// The reviewed inventory with one entry opted into refusing its target.
    fn opted_in(index: usize) -> (Inventory, Vec<Resolved>) {
        let (mut inventory, resolved) = reviewed(&CANDIDATES);
        inventory.hooks[index].refuses = true;
        (inventory, resolved)
    }

    #[test]
    fn an_opted_in_target_answers_a_boolean_and_every_other_one_still_answers_nothing() {
        // `server.tick` is the void candidate, so it is the one that may opt in.
        let (inventory, resolved) = opted_in(1);

        let shapes = shapes(&inventory, &resolved).expect("a void target may refuse");
        let java = hook_java(&adapter(), &shapes);
        let rust = exports(&shapes);

        assert!(
            java.contains("public static native boolean serverTick(Object argument0);"),
            "{java}"
        );
        assert!(
            java.contains(
                "\"serverTick\",\n                \"(Ljava/lang/Object;)Z\",\n                true)"
            ),
            "the opt-in reaches the target table beside the descriptor it changes: {java}"
        );
        assert!(
            rust.contains(") -> ::jni::sys::jboolean {\n    crate::guard::enter_refusable("),
            "{rust}"
        );

        // The other two candidates are untouched by their neighbour's opt-in.
        assert!(
            java.contains(
                "public static native void registryRegister(Object argument0, \
                 Object argument1, Object argument2);"
            ),
            "{java}"
        );
        assert!(
            java.contains("\"(Ljava/lang/Object;Z)V\",\n                false)"),
            "a target nobody opted in still answers nothing and says so: {java}"
        );
        assert_eq!(
            rust.matches("crate::guard::enter(").count(),
            2,
            "only the opted-in export takes the refusable path: {rust}"
        );
    }

    #[test]
    fn a_target_that_returns_a_value_is_refused_the_opt_in_rather_than_given_a_default() {
        // `registry.register` returns a `Holder$Reference`, so an early return
        // would have to invent one.
        let (inventory, resolved) = opted_in(0);

        let error = shapes(&inventory, &resolved)
            .expect_err("a value-returning target must not be refusable");

        let message = error.to_string();
        assert!(message.contains("registry.register"), "{message}");
        assert!(message.contains("does not return void"), "{message}");
        assert!(message.contains("invent a value"), "{message}");
    }

    /// One stub, written the way the resolver hands them over.
    fn stub(class: &str, flags: u16, members: &[(&str, &str, u16, bool)]) -> Stub {
        Stub {
            class: class.to_owned(),
            flags,
            extends: None,
            implements: Vec::new(),
            members: members
                .iter()
                .map(|(name, descriptor, flags, method)| StubMember {
                    name: (*name).to_owned(),
                    descriptor: (*descriptor).to_owned(),
                    flags: *flags,
                    method: *method,
                    returns: None,
                })
                .collect(),
            super_constructor: None,
        }
    }

    #[test]
    fn a_stub_declares_the_members_it_carries_with_the_descriptors_the_jar_declared() {
        let screen = stub(
            "com/example/client/gui/screens/Screen",
            0x0421, // public abstract class
            &[
                ("<init>", "(Lcom/example/network/chat/Component;)V", 4, true),
                ("init", "()V", 4, true),
                ("init", "(II)V", 0x0011, true),
                (
                    "getTitle",
                    "()Lcom/example/network/chat/Component;",
                    1,
                    true,
                ),
            ],
        );

        let (path, java) =
            stub_java(&adapter(), &screen, &Mapping::Identity).expect("a stub should render");

        assert_eq!(path, "com/example/client/gui/screens/Screen.java");
        assert!(
            java.contains("package com.example.client.gui.screens;"),
            "{java}"
        );
        assert!(java.contains("public abstract class Screen {"), "{java}");
        assert!(
            java.contains("protected Screen(com.example.network.chat.Component argument0) {"),
            "a constructor takes the class's simple name: {java}"
        );
        assert!(java.contains("protected void init() {"), "{java}");
        assert!(
            java.contains("public final void init(int argument0, int argument1) {"),
            "an overload is stubbed beside the one it overloads: {java}"
        );
        assert!(
            java.contains(
                "public com.example.network.chat.Component getTitle() {\n        \
                 throw new UnsupportedOperationException(\"stub\");"
            ),
            "a stub body is never reached and says so if it is: {java}"
        );
    }

    /// The defect the element type exists for: a stub generated from the erased
    /// descriptor alone declares a raw list, every element of it is an `Object`,
    /// and a caller that reads one back compiles against any type it likes.
    /// Rendering the element is what turns that into a compile error.
    #[test]
    fn a_java_util_return_is_declared_over_the_element_the_class_file_records() {
        let mut splitter = stub(
            "com/example/text/Splitter",
            0x0031,
            &[(
                "splitLines",
                "(Ljava/lang/String;)Ljava/util/List;",
                1,
                true,
            )],
        );
        splitter.members[0].returns = Some("Lcom/example/text/Line;".to_owned());

        let (_, java) = stub_java(&adapter(), &splitter, &Mapping::Identity).expect("renders");

        assert!(
            java.contains(
                "public java.util.List<com.example.text.Line> splitLines(java.lang.String \
                 argument0)"
            ),
            "{java}"
        );
    }

    /// A wildcard is the shape the pinned screens' own superclass declares, and
    /// a stub that dropped the bound would declare a list nothing assignable
    /// could be read out of.
    #[test]
    fn a_wildcard_element_keeps_the_bound_the_class_file_records() {
        let mut panel = stub(
            "com/example/gui/Panel",
            0x0031,
            &[("children", "()Ljava/util/List;", 1, true)],
        );
        panel.members[0].returns = Some("+Lcom/example/gui/Listener;".to_owned());

        let (_, java) = stub_java(&adapter(), &panel, &Mapping::Identity).expect("renders");

        assert!(
            java.contains("public java.util.List<? extends com.example.gui.Listener> children()"),
            "{java}"
        );
    }

    #[test]
    fn a_stubbed_supertype_is_declared_and_an_interface_method_gets_a_body() {
        let mut component = stub(
            "com/example/network/chat/Component",
            0x0601, // public abstract interface
            &[(
                "translatable",
                "(Ljava/lang/String;)Lcom/example/network/chat/MutableComponent;",
                0x0009,
                true,
            )],
        );
        component.implements = vec!["com/example/network/chat/FormattedText".to_owned()];
        let mut mutable = stub("com/example/network/chat/MutableComponent", 0x0031, &[]);
        mutable.implements = vec!["com/example/network/chat/Component".to_owned()];

        let (_, interface) =
            stub_java(&adapter(), &component, &Mapping::Identity).expect("renders");
        let (_, class) = stub_java(&adapter(), &mutable, &Mapping::Identity).expect("renders");

        assert!(
            interface.contains(
                "public interface Component extends com.example.network.chat.FormattedText {"
            ),
            "{interface}"
        );
        assert!(
            interface.contains(
                "static com.example.network.chat.MutableComponent \
                 translatable(java.lang.String argument0) {"
            ),
            "an interface member is never declared public, which javac warns on: {interface}"
        );
        assert!(
            class.contains(
                "public final class MutableComponent implements \
                 com.example.network.chat.Component {"
            ),
            "the edge that makes a returned value assignable is what the stub keeps: {class}"
        );
    }

    #[test]
    fn a_non_static_interface_method_is_given_a_body_rather_than_left_abstract() {
        let component = stub(
            "com/example/network/chat/Component",
            0x0601,
            &[("getString", "()Ljava/lang/String;", 0x0401, true)],
        );

        let (_, java) = stub_java(&adapter(), &component, &Mapping::Identity).expect("renders");

        assert!(
            java.contains("default java.lang.String getString() {"),
            "a stub that implements this interface must not have to implement it: {java}"
        );
    }

    #[test]
    fn a_stub_that_cannot_be_written_down_is_refused_rather_than_written_wrongly() {
        for (broken, expected) in [
            (
                stub("a/B", 0x0201, &[("VALUE", "I", 0x0019, false)]),
                "field on an interface",
            ),
            (
                stub("a/B", 1, &[("of", "(Q)V", 1, true)]),
                "not a type descriptor",
            ),
        ] {
            let error = stub_java(&adapter(), &broken, &Mapping::Identity)
                .expect_err("this must not render");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    /// The pinned version reaches every interactive widget through a nested type,
    /// so a stub
    /// set that could not name one would compile a screen with no button on it.
    /// The stub is a top-level class carrying the binary name, because the binary
    /// name is what the JVM links the screen against at runtime.
    #[test]
    fn a_nested_class_is_declared_under_the_binary_name_the_jvm_links_on() {
        let builder = stub(
            "com/example/client/gui/components/Button$Builder",
            0x0021,
            &[(
                "bounds",
                "(IIII)Lcom/example/client/gui/components/Button$Builder;",
                1,
                true,
            )],
        );

        let (path, java) = stub_java(&adapter(), &builder, &Mapping::Identity).expect("renders");

        assert!(
            path.ends_with("com/example/client/gui/components/Button$Builder.java"),
            "{path}"
        );
        assert!(java.contains("public class Button$Builder"), "{java}");
        assert!(
            java.contains(
                "public com.example.client.gui.components.Button$Builder bounds(int argument0,"
            ),
            "a nested type is named the same way wherever it appears: {java}"
        );
    }

    #[test]
    fn a_field_stub_keeps_its_declared_type_and_drops_nothing_else() {
        let font = stub(
            "com/example/client/gui/Font",
            1,
            &[("lineHeight", "I", 1, false), ("glyphs", "[[J", 4, false)],
        );

        let (_, java) = stub_java(&adapter(), &font, &Mapping::Identity).expect("renders");

        assert!(java.contains("public int lineHeight;"), "{java}");
        assert!(java.contains("protected long[][] glyphs;"), "{java}");
    }

    #[test]
    fn the_stub_lock_records_what_was_read_and_says_what_it_does_not_promise() {
        let mut screen = stub(
            "com/example/client/gui/screens/Screen",
            0x0421,
            &[("init", "()V", 4, true)],
        );
        screen.extends = Some("com/example/A".to_owned());
        screen.implements = vec!["com/example/B".to_owned()];

        let lock = stub_lock(&adapter(), &[screen]);

        assert!(lock.contains("extractor = \"classfile\""), "{lock}");
        assert!(
            lock.contains(
                "[[stub]]\nclass = \"com/example/client/gui/screens/Screen\"\n\
                           flags = 1057\nextends = \"com/example/A\"\n\
                           implements = [\"com/example/B\"]"
            ),
            "{lock}"
        );
        assert!(
            lock.contains(
                "[[stub.member]]\nname = \"init\"\ndescriptor = \"()V\"\nflags = 4\nmethod = true"
            ),
            "{lock}"
        );
        assert!(
            lock.contains("checked only where the pinned jars are"),
            "the header has to say what the lock is worth: {lock}"
        );
        assert!(
            lock.contains("dataFixerJarSha256 = "),
            "the lock records both sources' provenance: {lock}"
        );
        assert!(
            lock.contains("different table from mappings.lock"),
            "{lock}"
        );
    }

    #[test]
    fn a_descriptor_reads_back_as_the_java_types_a_stub_writes_down() {
        assert_eq!(
            signature("(Ljava/lang/String;[I)J").expect("valid"),
            (
                vec!["java.lang.String".to_owned(), "int[]".to_owned()],
                "long".to_owned()
            )
        );
        assert_eq!(
            signature("()V").expect("valid"),
            (Vec::new(), "void".to_owned())
        );
        assert_eq!(
            java_type("[[Ljava/lang/Object;").expect("valid"),
            ("java.lang.Object[][]".to_owned(), "")
        );
        assert!(
            signature("Ljava/lang/String;")
                .expect_err("this is not a method descriptor")
                .contains("not a method descriptor")
        );
        assert!(
            signature("()VV")
                .expect_err("one return type")
                .contains("more than one return type")
        );
        assert!(
            java_type("Lcom/example/A")
                .expect_err("unterminated")
                .contains("never terminated")
        );
        assert!(
            java_type("L;")
                .expect_err("names nothing")
                .contains("names no class")
        );
        assert!(
            java_type("")
                .expect_err("empty")
                .contains("ends before a type")
        );
    }

    #[test]
    fn one_identifier_becomes_a_constant_a_function_and_a_java_method() {
        assert_eq!(constant_name("chat.filter-text"), "CHAT_FILTER_TEXT");
        assert_eq!(rust_name("chat.filter-text"), "chat_filter_text");
        assert_eq!(java_name("chat.filter-text"), "chatFilterText");
        assert_eq!(
            java_name("input.smooth-last-amount"),
            "inputSmoothLastAmount"
        );
        assert_eq!(java_name(""), "");
    }
}
