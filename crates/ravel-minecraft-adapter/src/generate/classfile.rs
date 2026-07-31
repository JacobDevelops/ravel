//! Minimal class-file reader: enough to list a class's declared members.
//!
//! 26.1 ships unobfuscated class files, so the class file is the mapping. What
//! a binding generator needs from it is the exact name, descriptor, and access
//! flags of every field and method, which is the constant pool plus the two
//! member tables. Attributes are skipped by length; nothing here decodes code.

use std::fmt;

/// Class files start with this magic number.
const MAGIC: u32 = 0xCAFE_BABE;

/// Constant pool tag for a modified-UTF-8 string.
const TAG_UTF8: u8 = 1;
/// Constant pool tags that take two slots instead of one.
const TAG_LONG: u8 = 5;
const TAG_DOUBLE: u8 = 6;

/// Access flag marking a member as static.
const ACC_STATIC: u16 = 0x0008;

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
}

/// Reads every declared member of one class file.
pub(super) fn members(bytes: &[u8]) -> Result<Vec<Member>, ClassFileError> {
    let mut reader = Reader { bytes, at: 0 };
    if reader.u32()? != MAGIC {
        return Err(ClassFileError(
            "it does not start with the class file magic".to_owned(),
        ));
    }
    reader.skip(4)?; // minor and major version

    let strings = constant_pool(&mut reader)?;
    reader.skip(6)?; // access flags, this class, super class
    let interfaces = reader.u16()?;
    reader.skip(usize::from(interfaces) * 2)?;

    let mut members = Vec::new();
    for method in [false, true] {
        let count = reader.u16()?;
        for _ in 0..count {
            let flags = reader.u16()?;
            let name = strings.get(&mut reader)?;
            let descriptor = strings.get(&mut reader)?;
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

/// The constant pool's string entries, indexed as the class file indexes them.
struct Strings(Vec<Option<String>>);

impl Strings {
    /// Reads the next index and returns the string it names.
    fn get(&self, reader: &mut Reader<'_>) -> Result<String, ClassFileError> {
        let index = usize::from(reader.u16()?);
        self.0
            .get(index)
            .and_then(Option::as_ref)
            .cloned()
            .ok_or_else(|| ClassFileError(format!("constant pool entry {index} is not a string")))
    }
}

fn constant_pool(reader: &mut Reader<'_>) -> Result<Strings, ClassFileError> {
    let count = usize::from(reader.u16()?);
    let mut strings = vec![None; count.max(1)];
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
            TAG_LONG | TAG_DOUBLE => {
                reader.skip(8)?;
                // These occupy two pool slots, a quirk the format never dropped.
                index += 1;
            }
            _ => reader.skip(constant_width(tag)?)?,
        }
        index += 1;
    }
    Ok(Strings(strings))
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
    use super::{MAGIC, TAG_UTF8, members};

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
    fn an_unknown_constant_pool_tag_is_refused_rather_than_skipped() {
        let mut bytes = one_method_class();
        bytes[10] = 200; // the first constant pool tag

        let error = members(&bytes).expect_err("an unknown tag must not parse");

        assert!(error.to_string().contains("constant pool tag 200"));
    }
}
