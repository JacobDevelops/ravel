use std::collections::BTreeMap;
use std::path::Path;

use super::{Placeholder, Purpose, Template, Version, expand};
use crate::inputs::{Inventory, Seed, SeedMember, Seeds};
use crate::mapping::Mapping;
use crate::stubs::{Member, Stub};
use crate::templates::roles::Roles;
use crate::{GenerateError, test_fixtures};

fn template(text: &str, placeholders: Vec<Placeholder>) -> Template<'static> {
    Template {
        root: Path::new("templates"),
        path: Path::new("templates/Panel.java.in").to_owned(),
        name: "Panel".to_owned(),
        text: text.to_owned(),
        placeholders,
    }
}

fn with_version(test: impl FnOnce(&mut Version<'_>)) {
    let adapter = test_fixtures::legacy_adapter();
    let seeds = Seeds {
        schema_version: 1,
        minecraft: adapter.minecraft.clone(),
        stubs: vec![Seed {
            role: "Surface".to_owned(),
            class: "com.example.Panel".to_owned(),
            members: vec![SeedMember::Same("draw".to_owned())],
        }],
    };
    let inventory = Inventory {
        schema_version: 1,
        minecraft: adapter.minecraft.clone(),
        hooks: Vec::new(),
        non_goals: Vec::new(),
    };
    let roles = Roles::new(&seeds, &test_fixtures::reflection(&[]), &inventory);
    let stubs = [Stub {
        class: "com/example/Panel".to_owned(),
        flags: 1,
        extends: None,
        implements: Vec::new(),
        members: vec![Member {
            name: "draw".to_owned(),
            descriptor: "()V".to_owned(),
            flags: 1,
            method: true,
            returns: None,
        }],
        super_constructor: None,
    }];
    let mapping = Mapping::proguard("com.example.Panel -> game.Linked:\n    void draw() -> a\n")
        .expect("invented mapping");
    let linked = BTreeMap::from([("Surface".to_owned(), "game/Linked".to_owned())]);
    test(&mut Version {
        purpose: Purpose::Screen,
        adapter: &adapter,
        roles: &roles,
        stubs: &stubs,
        mapping: &mapping,
        linked: &linked,
        aliased: false,
        screens: Some("dev.ravel.screen"),
    });
}

#[test]
fn expansion_preserves_source_around_adjacent_and_repeated_placeholders() {
    with_version(|version| {
        let source = template(
            "// π\r\n{{import Surface}}\n{{Surface}}{{Surface}}.{{Surface#draw}}({{ravel Screen}});\n",
            vec![
                Placeholder::Import("Surface".to_owned()),
                Placeholder::Class("Surface".to_owned()),
                Placeholder::Class("Surface".to_owned()),
                Placeholder::Member("Surface".to_owned(), "draw".to_owned()),
                Placeholder::Ravel("Screen".to_owned()),
            ],
        );
        assert_eq!(
            expand(&source, version, None).expect("bound roles"),
            "// π\r\nimport game.Linked;\nLinkedLinked.a(dev.ravel.screen.Screen);\n"
        );
        let unchanged = template("// π\r\nclass Panel {}\n", Vec::new());
        assert_eq!(
            expand(&unchanged, version, None).expect("plain source"),
            unchanged.text
        );
    });
}

#[test]
fn imports_and_ravel_references_follow_the_available_namespace() {
    with_version(|version| {
        let template = template(
            "[{{import Surface}}] {{Surface}} {{ravel Screen}}",
            vec![
                Placeholder::Import("Surface".to_owned()),
                Placeholder::Class("Surface".to_owned()),
                Placeholder::Ravel("Screen".to_owned()),
            ],
        );
        version.aliased = true;
        version.screens = None;
        assert_eq!(
            expand(&template, version, None).expect("aliased class"),
            "[] Linked Screen"
        );
        let linked = BTreeMap::from([("Surface".to_owned(), "a".to_owned())]);
        let unnamed = Version {
            linked: &linked,
            aliased: false,
            ..*version
        };
        assert_eq!(
            expand(&template, &unnamed, None).expect("unnamed class"),
            "[] a Screen"
        );
    });
}

#[test]
fn package_provenance_stays_on_one_line_for_each_purpose() {
    with_version(|version| {
        let template = template("{{package}}\nclass Panel {}\n", vec![Placeholder::Package]);
        let prefix = format!(
            "Generated from the pinned Minecraft {} inputs. Do not edit.",
            version.adapter.minecraft
        );
        for purpose in [Purpose::Screen, Purpose::StandIn, Purpose::Driver] {
            version.purpose = purpose;
            assert_eq!(
                expand(&template, version, Some("dev.ravel.screen")).expect("named package"),
                format!("package dev.ravel.screen; // {prefix}\nclass Panel {{}}\n")
            );
            let reason = if purpose == Purpose::Driver {
                "A command line names this class, so it has no package."
            } else {
                "The game is in the unnamed package here, so this is too."
            };
            assert_eq!(
                expand(&template, version, None).expect("unnamed package"),
                format!("// {prefix} {reason}\nclass Panel {{}}\n")
            );
        }
    });
}

#[test]
fn unresolved_roles_report_the_template_path_and_keep_the_error_kind() {
    with_version(|version| {
        let template = template(
            "{{Missing}}",
            vec![Placeholder::Class("Missing".to_owned())],
        );
        let error = expand(&template, version, None).expect_err("unbound role");
        assert!(
            matches!(
                &error,
                GenerateError::Unresolved(detail)
                    if detail == "Panel.java.in: no linked name was resolved for the role Missing"
            ),
            "{error}"
        );
    });
}

#[test]
fn more_source_placeholders_than_reviewed_bindings_are_rejected() {
    with_version(|version| {
        let template = template(
            "{{Surface}}{{Surface}}",
            vec![Placeholder::Class("Surface".to_owned())],
        );
        let error = expand(&template, version, None).expect_err("missing reviewed binding");
        assert!(
            matches!(
                &error,
                GenerateError::Input(detail)
                    if detail == "Panel carries more placeholders than were read from it"
            ),
            "{error}"
        );
    });
}
