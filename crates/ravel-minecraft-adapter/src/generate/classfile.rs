//! Minimal class-file reader: enough to list a class's declared members and
//! name the types it sits under.
//!
//! 26.1 ships unobfuscated class files, so the class file is the mapping. What
//! a binding generator needs from it is the exact name, descriptor, and access
//! flags of every field and method, which is the constant pool plus the two
//! member tables. The stub generator needs one thing more: the superclass and
//! interfaces, which are named through `CONSTANT_Class` entries and so resolve
//! in two hops rather than one. Attributes are skipped by length; nothing here
//! decodes code.

use std::fmt;

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
            skip_attributes(&mut reader)?;
            members.push(Member {
                name,
                descriptor,
                flags,
                method,
            });
        }
    }
    Ok(members)
}

/// Reads the class's own flags and the types it sits under.
///
/// This is the part of the header `members` skips. A supertype is named through
/// a `CONSTANT_Class` entry rather than directly, so each name costs two hops:
/// the index in the header points at a class entry, and that entry points at the
/// string.
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

fn skip_attributes(reader: &mut Reader<'_>) -> Result<(), ClassFileError> {
    let count = reader.u16()?;
    for _ in 0..count {
        reader.skip(2)?; // name index
        let length = reader.u32()?;
        reader.skip(usize::try_from(length).unwrap_or(usize::MAX))?;
    }
    Ok(())
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

    /// The smallest class file that names a superclass and one interface.
    ///
    /// Entries 1 and 2 are the two internal names, entries 3 and 4 are the class
    /// references that point at them, which is the two-hop shape the header uses.
    fn one_hierarchy_class(flags: u16) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(MAGIC.to_be_bytes());
        bytes.extend(0_u16.to_be_bytes()); // minor
        bytes.extend(69_u16.to_be_bytes()); // major
        bytes.extend(5_u16.to_be_bytes()); // constant pool count: entries 1 to 4
        for text in ["some/Parent", "some/Contract"] {
            bytes.push(TAG_UTF8);
            bytes.extend(u16::try_from(text.len()).expect("short").to_be_bytes());
            bytes.extend(text.as_bytes());
        }
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
        let mut bytes = Vec::new();
        bytes.extend(MAGIC.to_be_bytes());
        bytes.extend(0_u16.to_be_bytes()); // minor
        bytes.extend(69_u16.to_be_bytes()); // major
        bytes.extend(3_u16.to_be_bytes()); // constant pool count: entries 1 and 2
        for text in ["hook", "(I)Z"] {
            bytes.push(TAG_UTF8);
            bytes.extend(u16::try_from(text.len()).expect("short").to_be_bytes());
            bytes.extend(text.as_bytes());
        }
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

    #[test]
    fn an_unknown_constant_pool_tag_is_refused_rather_than_skipped() {
        let mut bytes = one_method_class();
        bytes[10] = 200; // the first constant pool tag

        let error = members(&bytes).expect_err("an unknown tag must not parse");

        assert!(error.to_string().contains("constant pool tag 200"));
    }
}
