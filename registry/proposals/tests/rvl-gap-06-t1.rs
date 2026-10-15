// RVL-GAP-06, declaration-accepted.
//
// Original test case, written against the proposed Ravel capability.
//
// What it holds: one mod declares that it provides a shared library at a
// version, another declares that it needs that library within a range, and the
// launch resolves to exactly one provider. The case asks which provider was
// resolved rather than only that resolution succeeded, because a resolution that
// cannot be named is one nothing can be held to.

use ravel_sdk::{ModId, VersionRange, provision::{Provided, Provision}};

#[test]
fn one_provider_answers_one_requirement() {
    let provider = ModId::new("sample").expect("a well formed owner id");
    let consumer = ModId::new("probe").expect("a well formed owner id");
    let wanted = VersionRange::parse("^2").expect("a well formed range");

    let mut provision = Provision::default();
    provision.provides(&provider, Provided::new("shared_maths", "2.1.0"));
    provision.requires(&consumer, "shared_maths", wanted);

    let resolved = provision.resolve().expect("one provider resolves");

    assert_eq!(resolved.provider_of("shared_maths"), Some(&provider));
    assert_eq!(resolved.version_of("shared_maths"), Some("2.1.0"));
}

#[test]
fn a_provider_nobody_needs_is_still_resolved() {
    let provider = ModId::new("sample").expect("a well formed owner id");

    let mut provision = Provision::default();
    provision.provides(&provider, Provided::new("shared_maths", "2.1.0"));

    let resolved = provision.resolve().expect("an unused provider is not an error");

    assert_eq!(resolved.provider_of("shared_maths"), Some(&provider));
}
