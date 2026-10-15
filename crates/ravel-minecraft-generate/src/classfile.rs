//! Reads class files and owns shared JVM names, descriptor tokens, flags, and supertypes.

use std::fmt;

/// One validated JVM type, retaining its exact spelling for opaque handles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DescriptorType<'a> {
    pub(super) name: &'a str,
    pub(super) dimensions: usize,
    pub(super) descriptor: &'a str,
}

impl DescriptorType<'_> {
    fn source(self) -> String {
        let mut name = source_name(self.name);
        for _ in 0..self.dimensions {
            name.push_str("[]");
        }
        name
    }
}

/// Tokenizes exactly one JVM type; void is accepted only as a return type.
fn descriptor_type(descriptor: &str, returns: bool) -> Result<(DescriptorType<'_>, &str), String> {
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

/// Class files start with this magic number.
const MAGIC: u32 = 0xCAFE_BABE;

/// Constant pool tag for a modified-UTF-8 string.
const TAG_UTF8: u8 = 1;
/// Constant pool tag for a class reference, which names a string.
const TAG_CLASS: u8 = 7;
/// Constant pool tags that take two slots instead of one.
const TAG_LONG: u8 = 5;
const TAG_DOUBLE: u8 = 6;

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

/// The attribute a class file records an unerased declaration under.
const SIGNATURE: &str = "Signature";

/// Anything that stops a class file from being read.
#[derive(Debug)]
pub(super) struct ClassFileError(String);

impl fmt::Display for ClassFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "the class file is unreadable: {}", self.0)
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

/// Reads every declared member of one class file.
pub(super) fn members(bytes: &[u8]) -> Result<Vec<Member>, ClassFileError> {
    let (pool, mut reader) = header(bytes)?;
    reader.skip(6)?; // access flags, this class, super class
    let interfaces = reader.u16()?;
    reader.skip(usize::from(interfaces) * 2)?;

    let mut members = Vec::new();
    for method in [false, true] {
        let count = reader.u16()?;
        for _ in 0..count {
            let flags = reader.u16()?;
            let name = pool.string(&mut reader)?;
            let descriptor = pool.string(&mut reader)?;
            let signature = attributes(&pool, &mut reader)?;
            members.push(Member {
                name,
                descriptor,
                flags,
                method,
                signature,
            });
        }
    }
    Ok(members)
}

/// Reads the class's own flags and the types it sits under.
pub(super) fn hierarchy(bytes: &[u8]) -> Result<Hierarchy, ClassFileError> {
    let (pool, mut reader) = header(bytes)?;
    let flags = reader.u16()?;
    reader.skip(2)?; // this class
    let super_index = reader.u16()?;
    // Only `java/lang/Object` has no superclass, and it writes index zero.
    let super_class = if super_index == 0 {
        None
    } else {
        Some(pool.class(super_index)?)
    };

    let count = reader.u16()?;
    let mut interfaces = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        let index = reader.u16()?;
        interfaces.push(pool.class(index)?);
    }

    Ok(Hierarchy {
        super_class,
        interfaces,
        flags,
    })
}

/// Reads the magic, the version, and the constant pool, leaving the rest.
fn header(bytes: &[u8]) -> Result<(Pool, Reader<'_>), ClassFileError> {
    let mut reader = Reader { bytes, at: 0 };
    if reader.u32()? != MAGIC {
        return Err(ClassFileError(
            "it does not start with the class file magic".to_owned(),
        ));
    }
    reader.skip(4)?; // minor and major version
    let pool = constant_pool(&mut reader)?;
    Ok((pool, reader))
}

/// The constant pool entries a generator reads, indexed as the class file does.
struct Pool {
    /// Modified-UTF-8 entries.
    strings: Vec<Option<String>>,
    /// Class entries, each holding the pool index of its own name.
    classes: Vec<Option<u16>>,
}

impl Pool {
    /// Reads the next index and returns the string it names.
    fn string(&self, reader: &mut Reader<'_>) -> Result<String, ClassFileError> {
        let index = usize::from(reader.u16()?);
        self.strings
            .get(index)
            .and_then(Option::as_ref)
            .cloned()
            .ok_or_else(|| ClassFileError(format!("constant pool entry {index} is not a string")))
    }

    /// Resolves a class entry to the internal name it holds.
    fn class(&self, index: u16) -> Result<String, ClassFileError> {
        let name = self
            .classes
            .get(usize::from(index))
            .and_then(|entry| *entry)
            .ok_or_else(|| ClassFileError(format!("constant pool entry {index} is not a class")))?;
        self.strings
            .get(usize::from(name))
            .and_then(Option::as_ref)
            .cloned()
            .ok_or_else(|| ClassFileError(format!("class entry {index} names no string")))
    }
}

fn constant_pool(reader: &mut Reader<'_>) -> Result<Pool, ClassFileError> {
    let count = usize::from(reader.u16()?);
    let mut strings = vec![None; count.max(1)];
    let mut classes = vec![None; count.max(1)];
    let mut index = 1;
    while index < count {
        let tag = reader.u8()?;
        match tag {
            TAG_UTF8 => {
                let length = usize::from(reader.u16()?);
                let bytes = reader.take(length)?;
                // Modified UTF-8 differs from UTF-8 only for NUL and
                // supplementary characters, which member names do not use.
                strings[index] = Some(String::from_utf8_lossy(bytes).into_owned());
            }
            TAG_CLASS => classes[index] = Some(reader.u16()?),
            TAG_LONG | TAG_DOUBLE => {
                reader.skip(8)?;
                // These occupy two pool slots, a quirk the format never dropped.
                index += 1;
            }
            _ => reader.skip(constant_width(tag)?)?,
        }
        index += 1;
    }
    Ok(Pool { strings, classes })
}

/// Bytes following a constant pool tag, for the tags that are fixed width.
fn constant_width(tag: u8) -> Result<usize, ClassFileError> {
    match tag {
        7 | 8 | 16 | 19 | 20 => Ok(2), // Class, String, MethodType, Module, Package
        15 => Ok(3),                   // MethodHandle
        3 | 4 | 9 | 10 | 11 | 12 | 17 | 18 => Ok(4), // numbers, references, Dynamic
        unknown => Err(ClassFileError(format!(
            "constant pool tag {unknown} is unknown"
        ))),
    }
}

/// Walks one member's attribute table, answering its `Signature` where it has
/// one.
fn attributes(pool: &Pool, reader: &mut Reader<'_>) -> Result<Option<String>, ClassFileError> {
    let count = reader.u16()?;
    let mut signature = None;
    for _ in 0..count {
        let name = pool.string(reader)?;
        let length = usize::try_from(reader.u32()?).unwrap_or(usize::MAX);
        if name == SIGNATURE {
            // A `Signature` attribute is one pool index and nothing else, so one
            // of any other length is malformed rather than something to skip.
            if length != 2 {
                return Err(ClassFileError(format!(
                    "a Signature attribute is {length} bytes rather than two"
                )));
            }
            signature = Some(pool.string(reader)?);
        } else {
            reader.skip(length)?;
        }
    }
    Ok(signature)
}

struct Reader<'bytes> {
    bytes: &'bytes [u8],
    at: usize,
}

impl<'bytes> Reader<'bytes> {
    fn take(&mut self, count: usize) -> Result<&'bytes [u8], ClassFileError> {
        let end = self
            .at
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| ClassFileError("it ends before its members do".to_owned()))?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }

    fn skip(&mut self, count: usize) -> Result<(), ClassFileError> {
        self.take(count).map(|_| ())
    }

    fn u8(&mut self) -> Result<u8, ClassFileError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ClassFileError> {
        let bytes = self.take(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32, ClassFileError> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

#[cfg(test)]
mod tests {
    use super::{ACC_INTERFACE, MAGIC, TAG_CLASS, TAG_UTF8, hierarchy, members};

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
        let mut bytes = class_prefix(&["some/Parent", "some/Contract"], 2);
        for name in [1_u16, 2] {
            bytes.push(TAG_CLASS);
            bytes.extend(name.to_be_bytes());
        }
        bytes.extend(flags.to_be_bytes()); // access flags
        bytes.extend(0_u16.to_be_bytes()); // this class
        bytes.extend(3_u16.to_be_bytes()); // super class
        bytes.extend(1_u16.to_be_bytes()); // one interface
        bytes.extend(4_u16.to_be_bytes());
        bytes.extend(0_u16.to_be_bytes()); // fields
        bytes.extend(0_u16.to_be_bytes()); // methods
        bytes.extend(0_u16.to_be_bytes()); // class attributes
        bytes
    }

    /// Builds the smallest class file that declares one static method.
    fn one_method_class() -> Vec<u8> {
        let mut bytes = class_prefix(&["hook", "(I)Z"], 0);
        bytes.extend(0_u16.to_be_bytes()); // access flags
        bytes.extend(0_u16.to_be_bytes()); // this class
        bytes.extend(0_u16.to_be_bytes()); // super class
        bytes.extend(0_u16.to_be_bytes()); // interfaces
        bytes.extend(0_u16.to_be_bytes()); // fields
        bytes.extend(1_u16.to_be_bytes()); // methods
        bytes.extend(0x0008_u16.to_be_bytes()); // ACC_STATIC
        bytes.extend(1_u16.to_be_bytes()); // name index
        bytes.extend(2_u16.to_be_bytes()); // descriptor index
        bytes.extend(0_u16.to_be_bytes()); // method attributes
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

        assert!(error.to_string().contains("class file magic"));
    }

    #[test]
    fn a_truncated_class_file_is_refused_rather_than_guessed() {
        let mut bytes = one_method_class();
        bytes.truncate(bytes.len() - 4);

        let error = members(&bytes).expect_err("a truncated file must not parse");

        assert!(error.to_string().contains("ends before"));
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

        assert!(error.to_string().contains("is not a class"), "{error}");
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
            ],
            0,
        );
        for value in [0_u16, 0, 0, 0, 0] {
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

        assert!(error.to_string().contains("rather than two"), "{error}");
    }

    #[test]
    fn an_unknown_constant_pool_tag_is_refused_rather_than_skipped() {
        let mut bytes = one_method_class();
        bytes[10] = 200; // the first constant pool tag

        let error = members(&bytes).expect_err("an unknown tag must not parse");

        assert!(error.to_string().contains("constant pool tag 200"));
    }
}
