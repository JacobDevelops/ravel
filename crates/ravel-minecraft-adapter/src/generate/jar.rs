//! Minimal zip reader: enough to pull named entries out of the pinned jar.
//!
//! It reads the central directory and inflates only the entries it was asked
//! for, rather than starting a process per class. The inflate itself is
//! `miniz_oxide`, which is pure Rust and carries `#![forbid(unsafe_code)]`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::GenerateError;

/// End of central directory record.
const END_OF_CENTRAL_DIRECTORY: [u8; 4] = [b'P', b'K', 5, 6];
/// Local file header, which precedes every entry's bytes.
const LOCAL_FILE_HEADER: [u8; 4] = [b'P', b'K', 3, 4];
/// Central directory file header.
const CENTRAL_FILE_HEADER: [u8; 4] = [b'P', b'K', 1, 2];

/// Fixed width of a local file header before its name and extra field.
const LOCAL_HEADER_BYTES: usize = 30;
/// Fixed width of a central directory header before its name and extra field.
const CENTRAL_HEADER_BYTES: usize = 46;
/// The largest a zip comment can be, which bounds the search for the end record.
const MAX_COMMENT_BYTES: usize = 0xffff;

/// Compression method for an entry stored without compression.
const STORED: u16 = 0;
/// Compression method for a deflated entry, which is what a jar uses.
const DEFLATED: u16 = 8;

/// One entry, as the central directory describes it.
#[derive(Clone, Copy, Debug)]
struct Entry {
    method: u16,
    compressed: usize,
    uncompressed: usize,
    header: usize,
}

/// A zip archive held in memory, indexed by entry name.
pub(super) struct Archive {
    bytes: Vec<u8>,
    entries: BTreeMap<String, Entry>,
}

impl Archive {
    /// Reads an archive and indexes its central directory.
    pub(super) fn open(path: &Path) -> Result<Self, GenerateError> {
        let name = super::inputs::file_name(&path.to_string_lossy()).to_owned();
        let bytes = fs::read(path)
            .map_err(|error| GenerateError::Extraction(format!("cannot read {name}: {error}")))?;
        Self::read(bytes).map_err(|detail| GenerateError::Extraction(format!("{name}: {detail}")))
    }

    /// Indexes an archive already held in memory.
    fn read(bytes: Vec<u8>) -> Result<Self, String> {
        let end = end_of_central_directory(&bytes)?;
        let count = usize::from(u16::from_le_bytes([bytes[end + 10], bytes[end + 11]]));
        let mut at = u32_at(&bytes, end + 16)?;

        let mut entries = BTreeMap::new();
        for _ in 0..count {
            if bytes.get(at..at + CENTRAL_HEADER_BYTES).is_none()
                || bytes[at..at + 4] != CENTRAL_FILE_HEADER
            {
                return Err("its central directory ends before its entries do".to_owned());
            }
            let method = u16::from_le_bytes([bytes[at + 10], bytes[at + 11]]);
            let compressed = u32_at(&bytes, at + 20)?;
            let uncompressed = u32_at(&bytes, at + 24)?;
            let name_length = usize::from(u16::from_le_bytes([bytes[at + 28], bytes[at + 29]]));
            let extra_length = usize::from(u16::from_le_bytes([bytes[at + 30], bytes[at + 31]]));
            let comment_length = usize::from(u16::from_le_bytes([bytes[at + 32], bytes[at + 33]]));
            let header = u32_at(&bytes, at + 42)?;
            let start = at + CENTRAL_HEADER_BYTES;
            let name = bytes
                .get(start..start + name_length)
                .ok_or_else(|| "an entry name runs past the archive".to_owned())?;
            entries.insert(
                String::from_utf8_lossy(name).into_owned(),
                Entry {
                    method,
                    compressed,
                    uncompressed,
                    header,
                },
            );
            at = start + name_length + extra_length + comment_length;
        }
        Ok(Self { bytes, entries })
    }

    /// Whether the archive carries an entry under this exact name.
    ///
    /// A supertype walk needs this: the client jar carries the game's own
    /// classes and not the libraries it compiles against, so a walk that reached
    /// `java/lang/Object` or a Brigadier interface has left the jar rather than
    /// found a broken one.
    pub(super) fn holds(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    /// Every entry name the archive holds, in sorted order.
    ///
    /// Sorted because the index is a `BTreeMap`, which is what makes a caller
    /// that takes the first match take the same match on every machine.
    pub(super) fn names(&self) -> impl Iterator<Item = &str> + '_ {
        self.entries.keys().map(String::as_str)
    }

    /// Reads one entry back out, inflating it when it is deflated.
    pub(super) fn entry(&self, name: &str) -> Result<Vec<u8>, GenerateError> {
        self.read_entry(name)
            .map_err(|detail| GenerateError::Extraction(format!("{name}: {detail}")))
    }

    fn read_entry(&self, name: &str) -> Result<Vec<u8>, String> {
        let entry = self
            .entries
            .get(name)
            .ok_or_else(|| "the jar has no such entry".to_owned())?;
        let header = entry.header;
        if self
            .bytes
            .get(header..header + LOCAL_HEADER_BYTES)
            .is_none_or(|bytes| bytes[..4] != LOCAL_FILE_HEADER)
        {
            return Err("its local header is missing".to_owned());
        }
        // The name and extra field are repeated in the local header, and the
        // extra field is allowed to differ from the central directory's, so the
        // data offset has to come from here rather than from the index.
        let name_length = usize::from(u16::from_le_bytes([
            self.bytes[header + 26],
            self.bytes[header + 27],
        ]));
        let extra_length = usize::from(u16::from_le_bytes([
            self.bytes[header + 28],
            self.bytes[header + 29],
        ]));
        let start = header + LOCAL_HEADER_BYTES + name_length + extra_length;
        let data = self
            .bytes
            .get(start..start + entry.compressed)
            .ok_or_else(|| "its bytes run past the archive".to_owned())?;

        match entry.method {
            STORED => Ok(data.to_vec()),
            DEFLATED => {
                miniz_oxide::inflate::decompress_to_vec_with_limit(data, entry.uncompressed)
                    .map_err(|error| format!("it does not inflate: {error}"))
            }
            other => Err(format!("compression method {other} is not supported")),
        }
    }
}

/// Finds the end of central directory record, which is at a variable offset
/// because the archive may carry a comment after it.
fn end_of_central_directory(bytes: &[u8]) -> Result<usize, String> {
    let smallest = 22;
    if bytes.len() < smallest {
        return Err("it is too short to be an archive".to_owned());
    }
    let earliest = bytes.len().saturating_sub(smallest + MAX_COMMENT_BYTES);
    for at in (earliest..=bytes.len() - smallest).rev() {
        if bytes[at..at + 4] == END_OF_CENTRAL_DIRECTORY {
            return Ok(at);
        }
    }
    Err("it has no end of central directory record".to_owned())
}

/// Reads a little-endian `u32` as an offset into the archive.
fn u32_at(bytes: &[u8], at: usize) -> Result<usize, String> {
    let field = bytes
        .get(at..at + 4)
        .ok_or_else(|| "it ends before its central directory does".to_owned())?;
    Ok(
        usize::try_from(u32::from_le_bytes([field[0], field[1], field[2], field[3]]))
            .expect("archive offset fits in usize"),
    )
}

/// Archive fixtures shared with the resolver's tests.
#[cfg(test)]
pub(super) mod fixture {
    use super::{
        Archive, CENTRAL_FILE_HEADER, DEFLATED, END_OF_CENTRAL_DIRECTORY, LOCAL_FILE_HEADER,
    };

    /// Indexes an archive built in memory.
    pub(in crate::generate) fn parse(bytes: Vec<u8>) -> Result<Archive, String> {
        Archive::read(bytes)
    }

    /// Builds a one-entry archive, storing or deflating the content.
    pub(in crate::generate) fn archive(name: &str, content: &[u8], method: u16) -> Vec<u8> {
        let payload = if method == DEFLATED {
            miniz_oxide::deflate::compress_to_vec(content, 6)
        } else {
            content.to_vec()
        };
        let name_length = u16::try_from(name.len()).expect("short name");
        let compressed = u32::try_from(payload.len()).expect("small");
        let uncompressed = u32::try_from(content.len()).expect("small");

        let mut bytes = Vec::new();
        bytes.extend(LOCAL_FILE_HEADER);
        bytes.extend([0_u8; 4]); // version needed, flags
        bytes.extend(method.to_le_bytes());
        bytes.extend([0_u8; 8]); // time, date, crc
        bytes.extend(compressed.to_le_bytes());
        bytes.extend(uncompressed.to_le_bytes());
        bytes.extend(name_length.to_le_bytes());
        bytes.extend(0_u16.to_le_bytes()); // extra length
        bytes.extend(name.as_bytes());
        bytes.extend(&payload);

        let central = bytes.len();
        bytes.extend(CENTRAL_FILE_HEADER);
        bytes.extend([0_u8; 6]); // versions made and needed, flags
        bytes.extend(method.to_le_bytes());
        bytes.extend([0_u8; 8]); // time, date, crc
        bytes.extend(compressed.to_le_bytes());
        bytes.extend(uncompressed.to_le_bytes());
        bytes.extend(name_length.to_le_bytes());
        bytes.extend([0_u8; 8]); // extra, comment, disk, internal attributes
        bytes.extend(0_u32.to_le_bytes()); // external attributes
        bytes.extend(0_u32.to_le_bytes()); // local header offset
        bytes.extend(name.as_bytes());

        let directory = bytes.len() - central;
        bytes.extend(END_OF_CENTRAL_DIRECTORY);
        bytes.extend([0_u8; 4]); // disk numbers
        bytes.extend(1_u16.to_le_bytes()); // entries on this disk
        bytes.extend(1_u16.to_le_bytes()); // entries total
        bytes.extend(u32::try_from(directory).expect("small").to_le_bytes());
        bytes.extend(u32::try_from(central).expect("small").to_le_bytes());
        bytes.extend(0_u16.to_le_bytes()); // comment length
        bytes
    }

    /// Builds a one-entry archive that stores its content uncompressed.
    pub(in crate::generate) fn stored(name: &str, content: &[u8]) -> Vec<u8> {
        archive(name, content, super::STORED)
    }

    /// Builds a one-entry archive that deflates its content.
    pub(in crate::generate) fn deflated(name: &str, content: &[u8]) -> Vec<u8> {
        archive(name, content, DEFLATED)
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::{archive, deflated, parse, stored};

    #[test]
    fn a_deflated_entry_reads_back_as_the_bytes_that_went_in() {
        let content = b"the class file the generator wants to parse".repeat(40);
        let archive = parse(deflated("a/B.class", &content)).expect("an archive");

        assert_eq!(archive.entry("a/B.class").expect("the entry"), content);
    }

    #[test]
    fn a_stored_entry_reads_back_without_inflating() {
        let archive = parse(stored("plain.txt", b"stored")).expect("an archive");

        assert_eq!(archive.entry("plain.txt").expect("the entry"), b"stored");
    }

    #[test]
    fn an_entry_the_archive_does_not_carry_is_refused_by_name() {
        let archive = parse(stored("plain.txt", b"stored")).expect("an archive");

        let error = archive
            .entry("absent.class")
            .expect_err("this must not read");

        assert!(error.to_string().contains("absent.class"));
        assert!(error.to_string().contains("no such entry"));
    }

    #[test]
    fn an_unsupported_compression_method_is_refused_rather_than_guessed() {
        let archive = parse(archive("odd.txt", b"content", 99)).expect("an archive");

        assert!(
            archive
                .entry("odd.txt")
                .expect_err("this must not read")
                .to_string()
                .contains("compression method 99")
        );
    }

    #[test]
    fn bytes_that_are_not_an_archive_are_refused() {
        assert!(
            parse(b"not an archive at all, and long enough to be one".to_vec())
                .err()
                .expect("this must not parse")
                .contains("no end of central directory")
        );
        assert!(
            parse(b"short".to_vec())
                .err()
                .expect("this must not parse")
                .contains("too short")
        );
    }

    #[test]
    fn a_truncated_central_directory_is_refused_rather_than_guessed() {
        let mut bytes = stored("plain.txt", b"stored");
        let end = bytes.len() - 22;
        // Point the end record at a central directory that is not there.
        bytes[end + 16..end + 20].copy_from_slice(&u32::MAX.to_le_bytes());

        assert!(
            parse(bytes)
                .err()
                .expect("this must not parse")
                .contains("central directory")
        );
    }

    #[test]
    fn an_entry_whose_bytes_run_past_the_archive_is_refused() {
        let mut bytes = stored("plain.txt", b"stored");
        // Claim a compressed length the archive does not hold.
        bytes[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        let end = bytes.len() - 22;
        let central = super::u32_at(&bytes, end + 16).expect("the stored archive has an offset");
        bytes[central + 20..central + 24].copy_from_slice(&u32::MAX.to_le_bytes());

        let archive = parse(bytes).expect("the directory still indexes");

        assert!(
            archive
                .entry("plain.txt")
                .expect_err("this must not read")
                .to_string()
                .contains("run past the archive")
        );
    }
}
