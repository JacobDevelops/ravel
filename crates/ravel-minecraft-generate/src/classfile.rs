//! Reads class files and owns shared JVM names, descriptor tokens, flags, and supertypes.

use std::fmt;

use noak::MStr;
use noak::descriptor::{MethodDescriptor, TypeDescriptor};
use noak::error::{DecodeError, DecodeErrorKind};
use noak::reader::DecodeMany;
use noak::reader::attributes::AttributeContent;
use noak::reader::cpool::{ConstantPool, Index, Utf8};
use noak::reader::{Attribute, Class};

/// One validated JVM type, retaining its exact spelling for opaque handles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DescriptorType<'a> {
    pub(super) name: &'a str,
    pub(super) dimensions: usize,
    pub(super) descriptor: &'a str,
}

impl<'a> DescriptorType<'a> {
    pub(super) fn class_name(self) -> Option<&'a str> {
        self.descriptor
            .trim_start_matches('[')
            .starts_with('L')
            .then_some(self.name)
    }

    fn source(self) -> String {
        let mut name = source_name(self.name);
        for _ in 0..self.dimensions {
            name.push_str("[]");
        }
        name
    }
}

/// Tokenizes exactly one JVM type; void is accepted only as a return type.
pub(super) fn descriptor_type(
    descriptor: &str,
    returns: bool,
) -> Result<(DescriptorType<'_>, &str), String> {
    let dimensions = descriptor.bytes().take_while(|byte| *byte == b'[').count();
    let rest = &descriptor[dimensions..];
    let first = rest
        .chars()
        .next()
        .ok_or_else(|| format!("{descriptor} ends before a type does"))?;
    let (name, after) = match first {
        'V' if returns && dimensions == 0 => ("void", &rest[1..]),
        'Z' => ("boolean", &rest[1..]),
        'B' => ("byte", &rest[1..]),
        'C' => ("char", &rest[1..]),
        'S' => ("short", &rest[1..]),
        'I' => ("int", &rest[1..]),
        'J' => ("long", &rest[1..]),
        'F' => ("float", &rest[1..]),
        'D' => ("double", &rest[1..]),
        'L' => {
            let end = rest
                .find(';')
                .ok_or_else(|| format!("{descriptor} names a class that is never terminated"))?;
            let name = &rest[1..end];
            if name.is_empty() {
                return Err(format!("{descriptor} names no class between L and ;"));
            }
            (name, &rest[end + 1..])
        }
        other => return Err(format!("{other} is not a type descriptor")),
    };
    Ok((
        DescriptorType {
            name,
            dimensions,
            descriptor: &descriptor[..descriptor.len() - after.len()],
        },
        after,
    ))
}

/// Parses one complete non-void field descriptor, refusing trailing input.
pub(super) fn field_type(descriptor: &str) -> Result<DescriptorType<'_>, String> {
    let (kind, after) = descriptor_type(descriptor, false)?;
    if !after.is_empty() {
        return Err(format!("{descriptor} is not a field descriptor"));
    }
    Ok(kind)
}

/// Parses every parameter and exactly one return type with the same tokenizer.
pub(super) fn method_types(
    descriptor: &str,
) -> Result<(Vec<DescriptorType<'_>>, DescriptorType<'_>), String> {
    let mut rest = descriptor
        .strip_prefix('(')
        .ok_or_else(|| format!("{descriptor} is not a method descriptor"))?;
    let mut parameters = Vec::new();
    while !rest.starts_with(')') {
        let (kind, after) = descriptor_type(rest, false)?;
        parameters.push(kind);
        rest = after;
    }
    let (returns, after) = descriptor_type(&rest[1..], true)?;
    if !after.is_empty() {
        return Err(format!("{descriptor} declares more than one return type"));
    }
    Ok((parameters, returns))
}

/// Reads a method descriptor as Java source types.
pub(super) fn signature(descriptor: &str) -> Result<(Vec<String>, String), String> {
    let (parameters, returns) = method_types(descriptor)?;
    Ok((
        parameters.into_iter().map(DescriptorType::source).collect(),
        returns.source(),
    ))
}

/// Reads one field or return type as Java source, retaining any following input.
pub(super) fn java_type(descriptor: &str) -> Result<(String, &str), String> {
    let (kind, after) = descriptor_type(descriptor, true)?;
    Ok((kind.source(), after))
}

/// Splits an internal class name into its package and the name Java declares.
///
/// A nested class keeps its `$`: the declared name is the binary one, which is
/// what the JVM links against and what javac writes into the class file. A class
/// with no package at all is the unnamed one, which an obfuscated version puts
/// most of the game in.
pub(super) fn split(class: &str) -> (Option<String>, &str) {
    class
        .rsplit_once('/')
        .map_or((None, class), |(package, simple)| {
            (Some(package.replace('/', ".")), simple)
        })
}

/// Whether these linked names span the unnamed package and named ones at once.
pub(super) fn name_space_is_split<'a>(classes: impl IntoIterator<Item = &'a str>) -> bool {
    let (mut named, mut unnamed) = (false, false);
    for class in classes {
        if split(class).0.is_none() {
            unnamed = true;
        } else {
            named = true;
        }
    }
    named && unnamed
}

/// The internal name a source declares and names one linked class by.
///
/// Java source inside a package cannot name a class outside every package — the
/// language has no syntax for it — so a version that splits its game across the
/// two has no source form for the packaged half at all. Where it is split, every
/// class is written in the unnamed package instead, which is the only name space
/// both halves can be spelled in and leaves javac one name per class. The
/// package is put back into the compiled class files afterwards, so what the JVM
/// links is still the linked name.
///
/// The alias runs this way round rather than lifting the unnamed half into a
/// package of Ravel's own because the packaged half is the small one: an
/// obfuscated version leaves a handful of classes named and moves the rest out
/// of every package, so writing them all as the majority already is renames the
/// fewest classes and leaves the harness's compiled names the ones the adapter
/// resolves for all but those few.
pub(super) fn alias(class: &str, split_name_space: bool) -> String {
    if split_name_space {
        split(class).1.to_owned()
    } else {
        class.to_owned()
    }
}

/// An internal class name as Java source writes it.
pub(super) fn source_name(class: &str) -> String {
    class.replace('/', ".")
}

/// Access flag marking a member as static.
pub(super) const ACC_STATIC: u16 = 0x0008;
/// Access flags a stub reproduces from the class it stands in for.
pub(super) const ACC_PUBLIC: u16 = 0x0001;
pub(super) const ACC_PRIVATE: u16 = 0x0002;
pub(super) const ACC_PROTECTED: u16 = 0x0004;
pub(super) const ACC_FINAL: u16 = 0x0010;
pub(super) const ACC_INTERFACE: u16 = 0x0200;
pub(super) const ACC_ABSTRACT: u16 = 0x0400;
/// Access flag marking a member the compiler invented.
const ACC_SYNTHETIC: u16 = 0x1000;

/// Decodes modified UTF-8 without silently replacing unpaired surrogates.
fn text(value: &MStr) -> Result<String, ClassFileError> {
    value
        .chars()
        .collect::<Result<String, _>>()
        .map_err(|surrogate| {
            ClassFileError(format!(
                "unpaired surrogate {surrogate:#x} in a classfile string"
            ))
        })
}

/// Walks all attribute envelopes, including nested code and record attributes.
/// Unknown attributes remain opaque; a Signature must be exactly one CP index.
fn attributes<'a>(
    pool: &ConstantPool<'a>,
    entries: DecodeMany<'a, Attribute<'a>, u16>,
) -> Result<Option<&'a MStr>, ClassFileError> {
    let mut signature = None;
    for entry in entries {
        let entry = entry?;
        let name = pool.retrieve(entry.name())?;
        if name.as_bytes() == b"Signature" && entry.content().len() != 2 {
            return Err(ClassFileError(format!(
                "Signature attribute has length {} rather than two",
                entry.content().len()
            )));
        }
        if name.as_bytes() == b"LineNumberTable" {
            // noak 0.7 decodes start_pc as u32; JVMS 4.7.12 specifies u16.
            let content = entry.content();
            let valid = content.first_chunk::<2>().is_some_and(|count| {
                content.len() == 2 + usize::from(u16::from_be_bytes(*count)) * 4
            });
            if !valid {
                return Err(ClassFileError(
                    "LineNumberTable attribute has inconsistent length and entry count".to_owned(),
                ));
            }
            continue;
        }
        let content = match entry.read_content(pool) {
            Ok(content) => content,
            Err(error) if error.kind() == DecodeErrorKind::UnknownAttributeName => continue,
            Err(error) => return Err(error.into()),
        };
        if let AttributeContent::Signature(value) = content {
            signature = Some(pool.retrieve(value.signature())?);
        } else if let AttributeContent::Code(value) = content {
            attributes(pool, value.attributes())?;
        } else if let AttributeContent::Record(value) = content {
            for component in value.components() {
                let component = component?;
                pool.retrieve(component.name())?;
                TypeDescriptor::parse(pool.retrieve(component.descriptor())?)?;
                attributes(pool, component.attributes())?;
            }
        }
    }
    Ok(signature)
}

/// Anything that stops a class file from being read.
#[derive(Debug)]
pub(super) struct ClassFileError(String);

impl fmt::Display for ClassFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "the class file is unreadable: {}", self.0)
    }
}

impl From<DecodeError> for ClassFileError {
    fn from(error: DecodeError) -> Self {
        Self(error.to_string())
    }
}

/// One declared field or method.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Member {
    /// Declared name, including `<init>` and `<clinit>`.
    pub(super) name: String,
    /// Exact JVM descriptor.
    pub(super) descriptor: String,
    /// Declared access flags.
    pub(super) flags: u16,
    /// Whether this member is a method rather than a field.
    pub(super) method: bool,
    /// The `Signature` attribute, where the class file records one.
    ///
    /// A descriptor is erased and a signature is not, so this is the only place
    /// a class file says what a returned collection holds. Absent for every
    /// member whose declaration uses no type variable and no type argument,
    /// which is most of them.
    pub(super) signature: Option<String>,
}

impl Member {
    /// Whether the member is declared static.
    pub(super) const fn is_static(&self) -> bool {
        self.flags & ACC_STATIC != 0
    }

    /// Whether the member is reachable from outside its own package.
    ///
    /// A stub carries only these. A private member cannot be called by the code
    /// the stub exists to compile, and a package-private one cannot either,
    /// because nothing Ravel writes shares a package with a game class. A
    /// synthetic member has no source declaration to reproduce.
    pub(super) const fn is_visible(&self) -> bool {
        self.flags & (ACC_PUBLIC | ACC_PROTECTED) != 0
            && self.flags & (ACC_PRIVATE | ACC_SYNTHETIC) == 0
    }
}

/// Where one class sits: what it extends, what it implements, and what it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Hierarchy {
    /// Internal name of the superclass, absent only for `java/lang/Object`.
    pub(super) super_class: Option<String>,
    /// Internal names of the directly declared interfaces.
    pub(super) interfaces: Vec<String>,
    /// The class's own access flags.
    pub(super) flags: u16,
}

impl Hierarchy {
    /// Every type directly above this one, superclass first.
    pub(super) fn supertypes(&self) -> impl Iterator<Item = &str> {
        self.super_class
            .as_deref()
            .into_iter()
            .chain(self.interfaces.iter().map(String::as_str))
    }
}

fn member<'a>(
    pool: &ConstantPool<'a>,
    name: Index<Utf8<'a>>,
    descriptor: Index<Utf8<'a>>,
    flags: u16,
    method: bool,
    entries: DecodeMany<'a, Attribute<'a>, u16>,
) -> Result<Member, ClassFileError> {
    let descriptor = pool.retrieve(descriptor)?;
    if method {
        MethodDescriptor::parse(descriptor)?;
    } else {
        TypeDescriptor::parse(descriptor)?;
    }
    Ok(Member {
        name: text(pool.retrieve(name)?)?,
        descriptor: text(descriptor)?,
        flags,
        method,
        signature: attributes(pool, entries)?.map(text).transpose()?,
    })
}

/// Parses one complete class, validating hierarchy references and class attributes.
fn parse(bytes: &[u8]) -> Result<Class<'_>, ClassFileError> {
    let class = Class::new(bytes)?;
    if class.buffer_size() != bytes.len() {
        return Err(ClassFileError(
            "Extra bytes after reading class file".to_owned(),
        ));
    }
    let pool = class.pool();
    pool.retrieve(class.this_class())?;
    if let Some(super_class) = class.super_class() {
        pool.retrieve(super_class)?;
    }
    for interface in class.interfaces() {
        pool.retrieve(interface?)?;
    }
    attributes(pool, class.attributes())?;
    Ok(class)
}

/// Reads fields first, then methods, retaining each table's declaration order.
pub(super) fn members(bytes: &[u8]) -> Result<Vec<Member>, ClassFileError> {
    let class = parse(bytes)?;
    let pool = class.pool();
    let fields = class.fields();
    let methods = class.methods();
    let capacity =
        fields.iter().size_hint().1.unwrap_or(0) + methods.iter().size_hint().1.unwrap_or(0);
    let mut members = Vec::with_capacity(capacity);
    for field in fields {
        let field = field?;
        members.push(member(
            pool,
            field.name(),
            field.descriptor(),
            field.access_flags().bits(),
            false,
            field.attributes(),
        )?);
    }
    for method in methods {
        let method = method?;
        members.push(member(
            pool,
            method.name(),
            method.descriptor(),
            method.access_flags().bits(),
            true,
            method.attributes(),
        )?);
    }
    Ok(members)
}

/// Reads hierarchy metadata without materializing unused member strings.
pub(super) fn hierarchy(bytes: &[u8]) -> Result<Hierarchy, ClassFileError> {
    let class = parse(bytes)?;
    let pool = class.pool();
    let super_class = class
        .super_class()
        .map(|index| text(pool.retrieve(index)?.name))
        .transpose()?;
    let interfaces = class
        .interfaces()
        .into_iter()
        .map(|index| text(pool.retrieve(index?)?.name))
        .collect::<Result<Vec<_>, ClassFileError>>()?;
    Ok(Hierarchy {
        super_class,
        interfaces,
        flags: class.access_flags().bits(),
    })
}

#[cfg(test)]
mod tests {
    use super::{ACC_INTERFACE, hierarchy, members};

    const MAGIC: u32 = 0xCAFE_BABE;
    const TAG_CLASS: u8 = 7;
    const TAG_UTF8: u8 = 1;

    fn class_prefix(strings: &[&str], other_entries: u16) -> Vec<u8> {
        let size = 10 + strings.iter().map(|text| text.len() + 3).sum::<usize>();
        let mut bytes = Vec::with_capacity(size);
        bytes.extend(MAGIC.to_be_bytes());
        bytes.extend(0_u16.to_be_bytes()); // minor
        bytes.extend(69_u16.to_be_bytes()); // major
        let count = u16::try_from(strings.len()).expect("short") + other_entries + 1;
        bytes.extend(count.to_be_bytes());
        for text in strings {
            bytes.push(TAG_UTF8);
            bytes.extend(u16::try_from(text.len()).expect("short").to_be_bytes());
            bytes.extend(text.as_bytes());
        }
        bytes
    }

    fn one_hierarchy_class(flags: u16) -> Vec<u8> {
        let mut bytes = class_prefix(&["some/Parent", "some/Contract", "some/Child"], 3);
        for name in [1_u16, 2, 3] {
            bytes.push(TAG_CLASS);
            bytes.extend(name.to_be_bytes());
        }
        bytes.extend(flags.to_be_bytes()); // access flags
        bytes.extend(6_u16.to_be_bytes()); // this class
        bytes.extend(4_u16.to_be_bytes()); // super class
        bytes.extend(1_u16.to_be_bytes()); // one interface
        bytes.extend(5_u16.to_be_bytes());
        bytes.extend(0_u16.to_be_bytes()); // fields
        bytes.extend(0_u16.to_be_bytes()); // methods
        bytes.extend(0_u16.to_be_bytes()); // class attributes
        bytes
    }

    /// Builds the smallest class file that declares one static method.
    fn one_method_class() -> Vec<u8> {
        one_member_class("(I)Z", true)
    }

    fn one_member_class(descriptor: &str, method: bool) -> Vec<u8> {
        let mut bytes = class_prefix(&["hook", descriptor, "some/Child"], 1);
        bytes.push(TAG_CLASS);
        bytes.extend(3_u16.to_be_bytes());
        bytes.extend(0_u16.to_be_bytes()); // access flags
        bytes.extend(4_u16.to_be_bytes()); // this class
        bytes.extend(0_u16.to_be_bytes()); // super class
        bytes.extend(0_u16.to_be_bytes()); // interfaces
        if method {
            bytes.extend(0_u16.to_be_bytes()); // no fields
        }
        bytes.extend(1_u16.to_be_bytes()); // one field or method
        bytes.extend(0x0008_u16.to_be_bytes()); // ACC_STATIC
        bytes.extend(1_u16.to_be_bytes()); // name index
        bytes.extend(2_u16.to_be_bytes()); // descriptor index
        bytes.extend(0_u16.to_be_bytes()); // method attributes
        if !method {
            bytes.extend(0_u16.to_be_bytes()); // no methods
        }
        bytes.extend(0_u16.to_be_bytes()); // class attributes
        bytes
    }

    #[test]
    fn a_declared_static_method_is_read_with_its_descriptor() {
        let read = members(&one_method_class()).expect("the fixture should parse");

        assert_eq!(read.len(), 1);
        assert_eq!(read[0].name, "hook");
        assert_eq!(read[0].descriptor, "(I)Z");
        assert!(read[0].is_static());
        assert!(read[0].method);
    }

    #[test]
    fn bytes_that_are_not_a_class_file_are_refused() {
        let error = members(b"not a class file").expect_err("this must not parse");

        assert!(error.to_string().contains("invalid file prefix"), "{error}");
    }

    #[test]
    fn a_truncated_class_file_is_refused_rather_than_guessed() {
        let mut bytes = one_method_class();
        bytes.truncate(bytes.len() - 4);

        let error = members(&bytes).expect_err("a truncated file must not parse");

        assert!(
            error.to_string().contains("unexpected end of input"),
            "{error}"
        );
    }

    #[test]
    fn a_supertype_resolves_through_the_class_entry_that_names_it() {
        let read = hierarchy(&one_hierarchy_class(0)).expect("the fixture should parse");

        assert_eq!(read.super_class.as_deref(), Some("some/Parent"));
        assert_eq!(read.interfaces, ["some/Contract"]);
        assert_eq!(
            read.supertypes().collect::<Vec<_>>(),
            ["some/Parent", "some/Contract"],
            "a supertype walk sees the superclass and every interface"
        );
    }

    #[test]
    fn a_class_carries_its_own_flags_so_a_stub_can_say_what_it_is() {
        let read = hierarchy(&one_hierarchy_class(ACC_INTERFACE)).expect("the fixture parses");

        assert_eq!(read.flags, ACC_INTERFACE);
    }

    #[test]
    fn a_class_with_no_superclass_reports_none_rather_than_failing() {
        let mut bytes = one_hierarchy_class(0);
        // The superclass index sits after the pool, the flags, and this class.
        let at = bytes.len() - 12;
        bytes[at..at + 2].copy_from_slice(&0_u16.to_be_bytes());

        let read = hierarchy(&bytes).expect("only java/lang/Object writes index zero");

        assert_eq!(read.super_class, None);
    }

    #[test]
    fn a_header_pointing_at_something_that_is_not_a_class_is_refused() {
        let mut bytes = one_hierarchy_class(0);
        let at = bytes.len() - 12;
        // Entry 1 is a string, not a class reference.
        bytes[at..at + 2].copy_from_slice(&1_u16.to_be_bytes());

        let error = hierarchy(&bytes).expect_err("a string is not a supertype");

        assert!(error.to_string().contains("tag mismatch"), "{error}");
    }

    #[test]
    fn only_a_member_callable_from_outside_its_package_reaches_a_stub() {
        let read = members(&one_method_class()).expect("the fixture should parse");

        assert!(
            !read[0].is_visible(),
            "the fixture declares the method package-private"
        );
    }

    /// The smallest class file whose one method carries a `Signature`, between
    /// two attributes that are not one.
    fn one_signed_method_class(signature_length: u32) -> Vec<u8> {
        let mut bytes = class_prefix(
            &[
                "hook",
                "()Ljava/util/List;",
                "Unrelated",
                "Signature",
                "()Ljava/util/List<Lsome/Line;>;",
                "some/Child",
            ],
            1,
        );
        bytes.push(TAG_CLASS);
        bytes.extend(6_u16.to_be_bytes());
        for value in [0_u16, 7, 0, 0, 0] {
            // flags, this class, no superclass, no interfaces, no fields
            bytes.extend(value.to_be_bytes());
        }
        bytes.extend(1_u16.to_be_bytes()); // one method
        bytes.extend(1_u16.to_be_bytes()); // ACC_PUBLIC
        bytes.extend(1_u16.to_be_bytes()); // name index
        bytes.extend(2_u16.to_be_bytes()); // descriptor index
        bytes.extend(3_u16.to_be_bytes()); // three attributes
        bytes.extend(3_u16.to_be_bytes()); // one named something else and four
        bytes.extend(4_u32.to_be_bytes()); // bytes wide, so a walk that skipped
        bytes.extend(0_u32.to_be_bytes()); // by anything but its length desyncs
        bytes.extend(4_u16.to_be_bytes()); // the Signature attribute
        bytes.extend(signature_length.to_be_bytes());
        bytes.extend(5_u16.to_be_bytes());
        let padding = usize::try_from(signature_length)
            .expect("short")
            .saturating_sub(2);
        bytes.extend(std::iter::repeat_n(0_u8, padding));
        bytes.extend(3_u16.to_be_bytes()); // and a last one two bytes wide like
        bytes.extend(2_u32.to_be_bytes()); // a signature, whose payload would
        bytes.extend(1_u16.to_be_bytes()); // read back as "hook"
        bytes.extend(0_u16.to_be_bytes()); // class attributes
        bytes
    }

    #[test]
    fn a_signature_is_found_past_an_attribute_that_is_not_one() {
        let read = members(&one_signed_method_class(2)).expect("the fixture should parse");

        assert_eq!(read.len(), 1);
        assert_eq!(read[0].name, "hook");
        assert_eq!(
            read[0].signature.as_deref(),
            Some("()Ljava/util/List<Lsome/Line;>;"),
            "the walk names each attribute through the pool and keeps only the signature"
        );
    }

    #[test]
    fn a_signature_attribute_of_another_width_is_refused_rather_than_skipped() {
        let error =
            members(&one_signed_method_class(3)).expect_err("a three-byte signature is malformed");

        assert!(
            error.to_string().contains("length 3 rather than two"),
            "{error}"
        );
        assert!(error.to_string().contains("Signature attribute"), "{error}");
    }

    #[test]
    fn an_unknown_constant_pool_tag_is_refused_rather_than_skipped() {
        let mut bytes = one_method_class();
        bytes[10] = 200; // the first constant pool tag

        let error = members(&bytes).expect_err("an unknown tag must not parse");

        assert!(error.to_string().contains("invalid tag"), "{error}");
    }

    #[test]
    fn modified_utf8_member_names_decode_without_replacement_characters() {
        for (encoded, expected) in [
            (&b"m\xc0\x80"[..], "m\0"),
            (&b"m\xed\xa0\x81\xed\xb0\x80"[..], "m\u{10400}"),
        ] {
            let mut bytes = one_method_class();
            bytes[11..13].copy_from_slice(
                &u16::try_from(encoded.len())
                    .expect("a short name")
                    .to_be_bytes(),
            );
            bytes.splice(13..17, encoded.iter().copied());

            let read = members(&bytes).expect("modified UTF-8 is the classfile encoding");

            assert_eq!(read[0].name, expected);
        }
    }

    #[test]
    fn malformed_descriptors_are_not_reformatted_into_valid_members() {
        for (descriptor, method) in [
            ("(I)ZZ", true),
            ("(I)Vextra", true),
            ("Iextra", false),
            ("V", false),
        ] {
            let error = members(&one_member_class(descriptor, method))
                .expect_err("the complete raw descriptor must be validated");
            assert!(
                error.to_string().contains("invalid descriptor"),
                "{descriptor}: {error}"
            );
        }
    }

    #[test]
    fn extra_bytes_after_a_class_are_refused() {
        let mut bytes = one_method_class();
        bytes.push(0);

        let error = members(&bytes).expect_err("a complete class cannot have a trailing byte");

        assert!(error.to_string().contains("Extra bytes"), "{error}");
    }

    #[test]
    fn malformed_nested_code_attributes_are_not_hidden_by_a_valid_outer_length() {
        let mut bytes = class_prefix(&["hook", "()V", "Code", "some/Child"], 1);
        bytes.push(TAG_CLASS);
        bytes.extend(4_u16.to_be_bytes());
        for value in [0_u16, 5, 0, 0, 0, 1, 1, 1, 2, 1, 3] {
            bytes.extend(value.to_be_bytes());
        }
        bytes.extend(12_u32.to_be_bytes());
        bytes.extend(0_u16.to_be_bytes()); // max stack
        bytes.extend(0_u16.to_be_bytes()); // max locals
        bytes.extend(0_u32.to_be_bytes()); // code length
        bytes.extend(0_u16.to_be_bytes()); // exception handlers
        bytes.extend(1_u16.to_be_bytes()); // missing nested attribute
        bytes.extend(0_u16.to_be_bytes()); // class attributes, outside Code

        let error = members(&bytes).expect_err("nested attribute envelopes must be traversed");
        assert!(
            error.to_string().contains("unexpected end of input"),
            "{error}"
        );
    }

    fn one_line_table_class(content: &[u8]) -> Vec<u8> {
        let mut bytes = class_prefix(&["hook", "()V", "Code", "LineNumberTable", "some/Child"], 1);
        bytes.push(TAG_CLASS);
        bytes.extend(5_u16.to_be_bytes());
        for value in [0_u16, 6, 0, 0, 0, 1, 1, 1, 2, 1, 3] {
            bytes.extend(value.to_be_bytes());
        }
        bytes.extend((19 + u32::try_from(content.len()).expect("short table")).to_be_bytes());
        bytes.extend(0_u16.to_be_bytes()); // max stack
        bytes.extend(1_u16.to_be_bytes()); // max locals
        bytes.extend(1_u32.to_be_bytes()); // code length
        bytes.push(0xb1); // return
        bytes.extend(0_u16.to_be_bytes()); // exception handlers
        bytes.extend(1_u16.to_be_bytes()); // nested attribute count
        bytes.extend(4_u16.to_be_bytes()); // LineNumberTable
        bytes.extend(
            u32::try_from(content.len())
                .expect("short table")
                .to_be_bytes(),
        );
        bytes.extend(content);
        bytes.extend(0_u16.to_be_bytes()); // class attributes
        bytes
    }

    #[test]
    fn line_number_table_uses_two_u16_values_per_entry() {
        let bytes = one_line_table_class(&[0, 1, 0, 0, 0x12, 0x34]);
        let read = members(&bytes).expect("a valid javac line table uses four-byte entries");

        assert_eq!(read.len(), 1);
        assert_eq!(read[0].name, "hook");
        assert_eq!(read[0].descriptor, "()V");
    }

    #[test]
    fn line_number_table_refuses_missing_truncated_and_extra_entries() {
        for content in [
            &[0][..],
            &[0, 1, 0, 0, 0x12][..],
            &[0, 0, 0, 0][..],
            &[0, 2, 0, 0, 0x12, 0x34][..],
        ] {
            let error = members(&one_line_table_class(content))
                .expect_err("the complete table must match its declared entry count");

            assert!(
                error.to_string().contains("LineNumberTable attribute"),
                "{error}"
            );
        }
    }

    #[test]
    fn fields_precede_methods_and_each_keeps_its_declaration_order() {
        let mut bytes = class_prefix(&["z", "a", "I", "()V", "some/Child"], 1);
        bytes.push(TAG_CLASS);
        bytes.extend(5_u16.to_be_bytes());
        for value in [0_u16, 6, 0, 0] {
            bytes.extend(value.to_be_bytes());
        }
        for descriptor in [3_u16, 4] {
            bytes.extend(2_u16.to_be_bytes());
            for name in [1_u16, 2] {
                for value in [1_u16, name, descriptor, 0] {
                    bytes.extend(value.to_be_bytes());
                }
            }
        }
        bytes.extend(0_u16.to_be_bytes());

        let read = members(&bytes).expect("both member tables are valid");
        let order: Vec<_> = read
            .iter()
            .map(|member| (member.name.as_str(), member.method))
            .collect();

        assert_eq!(
            order,
            [("z", false), ("a", false), ("z", true), ("a", true)]
        );
    }

    #[test]
    fn modern_record_attributes_are_parsed_and_truncation_is_refused() {
        let mut bytes = class_prefix(&["some/Record", "Record", "value", "I"], 1);
        bytes.push(TAG_CLASS);
        bytes.extend(1_u16.to_be_bytes());
        for value in [0_u16, 5, 0, 0, 0, 0, 1, 2] {
            bytes.extend(value.to_be_bytes());
        }
        bytes.extend(8_u32.to_be_bytes());
        for value in [1_u16, 3, 4, 0] {
            bytes.extend(value.to_be_bytes());
        }

        assert_eq!(members(&bytes).expect("a modern record attribute").len(), 0);
        let count_at = bytes.len() - 8;
        bytes[count_at..count_at + 2].copy_from_slice(&2_u16.to_be_bytes());
        assert!(
            members(&bytes).is_err(),
            "a record cannot claim a missing second component"
        );
        bytes[count_at..count_at + 2].copy_from_slice(&1_u16.to_be_bytes());
        bytes.pop();
        assert!(
            members(&bytes).is_err(),
            "a truncated record must be refused"
        );
    }

    #[test]
    fn modern_module_attributes_are_supported() {
        let mut bytes = class_prefix(&["module-info", "Module", "example.module"], 2);
        bytes.push(TAG_CLASS);
        bytes.extend(1_u16.to_be_bytes());
        bytes.push(19); // CONSTANT_Module
        bytes.extend(3_u16.to_be_bytes());
        for value in [0x8000_u16, 4, 0, 0, 0, 0, 1, 2] {
            bytes.extend(value.to_be_bytes());
        }
        bytes.extend(16_u32.to_be_bytes());
        for value in [5_u16, 0, 0, 0, 0, 0, 0, 0] {
            bytes.extend(value.to_be_bytes());
        }

        assert_eq!(members(&bytes).expect("a modern module attribute").len(), 0);
        assert_eq!(
            hierarchy(&bytes)
                .expect("a module has no parent")
                .super_class,
            None
        );
    }
}
