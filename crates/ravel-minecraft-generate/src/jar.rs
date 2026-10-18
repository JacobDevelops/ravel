//! Read-only JAR access with sorted names and bounded entry decompression.

use std::borrow::Cow;
use std::collections::{BTreeMap, btree_map::Entry};
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::Arc;

use zip::ZipArchive;
use zip::read::{ArchiveOffset, Config, ZipFile, ZipFileEntry};
use zip::result::{ZipError, ZipResult};
use zip::unstable::stream::{ZipStreamReader, ZipStreamVisitor};

use super::GenerateError;

/// A cheap-to-clone reader: archive bytes and parsed metadata are shared.
type JarReader = ZipArchive<Cursor<Arc<[u8]>>>;

/// A zip archive held in memory, indexed by entry name.
pub(super) struct Archive {
    reader: JarReader,
    entries: BTreeMap<String, usize>,
    byte_len: u64,
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
        if bytes.len() < 22 {
            return Err("the jar is too short to carry an end of central directory".to_owned());
        }
        let bytes: Arc<[u8]> = bytes.into();
        let byte_len = u64::try_from(bytes.len()).map_err(|error| error.to_string())?;
        let mut reader = ZipArchive::with_config(
            Config {
                archive_offset: ArchiveOffset::Known(0),
            },
            Cursor::new(Arc::clone(&bytes)),
        )
        .map_err(|error| format!("cannot read central directory: {error}"))?;
        reject_duplicate_entries(&reader, bytes)?;
        for index in 0..reader.len() {
            let file = reader
                .by_index_raw(index)
                .map_err(|error| error.to_string())?;
            if file
                .extra_data_fields()
                .any(|field| matches!(field, zip::ExtraField::UnicodePath(_)))
            {
                return Err("Unicode path extra fields are not supported in game jars".to_owned());
            }
            if file.size() > u64::from(u32::MAX) {
                return Err("entry exceeds the supported 32-bit uncompressed size".to_owned());
            }
        }
        let mut entries = BTreeMap::new();
        for (index, name) in reader.file_names().enumerate() {
            let name = name.map_err(|error| error.to_string())?.into_owned();
            match entries.entry(name) {
                Entry::Vacant(entry) => {
                    entry.insert(index);
                }
                Entry::Occupied(entry) => {
                    return Err(format!("the jar has duplicate entry {}", entry.key()));
                }
            }
        }
        Ok(Self {
            reader,
            entries,
            byte_len,
        })
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
        let index = *self
            .entries
            .get(name)
            .ok_or_else(|| "the jar has no such entry".to_owned())?;
        let mut reader = self.reader.clone();
        let mut file = reader.by_index(index).map_err(|error| error.to_string())?;
        if file
            .data_start()
            .and_then(|start| start.checked_add(file.compressed_size()))
            .is_none_or(|end| end > self.byte_len)
        {
            return Err("its bytes run past the archive".to_owned());
        }
        let declared = file.size();
        let mut bytes = Vec::new();
        // Never trust the declared size for an up-front allocation, and never
        // let a forged small size inflate into an unbounded output buffer.
        file.by_ref()
            .take(declared)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        // Force the inflater/CRC reader to check EOF, including its size cap.
        file.read(&mut [0_u8; 1])
            .map_err(|error| error.to_string())?;
        if u64::try_from(bytes.len()).map_err(|error| error.to_string())? != declared {
            return Err("entry ends before its declared size".to_owned());
        }
        Ok(bytes)
    }
}

/// `ZipArchive` collapses duplicate raw names. Visit only central metadata,
/// without opening or decompressing any local entries, to detect that loss.
fn reject_duplicate_entries(reader: &JarReader, bytes: Arc<[u8]>) -> Result<(), String> {
    if reader.is_empty() {
        return Ok(());
    }
    let mut cursor = Cursor::new(bytes);
    cursor.set_position(reader.central_directory_start());
    let mut counter = CentralEntries(0);
    ZipStreamReader::new(cursor)
        .visit(&mut counter)
        .map_err(|error| error.to_string())?;
    if counter.0 != reader.len() {
        return Err("the jar has duplicate entries in its central directory".to_owned());
    }
    Ok(())
}

struct CentralEntries(usize);

impl ZipStreamVisitor for CentralEntries {
    fn visit_file<R: Read>(&mut self, _file: &mut ZipFile<'_, R>) -> ZipResult<()> {
        Err(ZipError::InvalidArchive(Cow::Borrowed(
            "local entry found in the central directory",
        )))
    }

    fn visit_additional_metadata(&mut self, _metadata: &ZipFileEntry<'_>) -> ZipResult<()> {
        self.0 += 1;
        Ok(())
    }
}

/// Archive fixtures shared with the resolver's tests.
#[cfg(test)]
pub(super) mod fixture {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    use super::Archive;

    /// Indexes an archive built in memory.
    pub(crate) fn parse(bytes: Vec<u8>) -> Result<Archive, String> {
        Archive::read(bytes)
    }

    /// Builds a one-entry archive, storing or deflating the content.
    pub(crate) fn archive(name: &str, content: &[u8], method: u16) -> Vec<u8> {
        let compression = if method == 8 {
            CompressionMethod::Deflated
        } else {
            CompressionMethod::Stored
        };
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(compression),
            )
            .expect("a valid fixture entry");
        writer.write_all(content).expect("writing to memory");
        let mut bytes = writer.finish().expect("a complete archive").into_inner();
        if !matches!(method, 0 | 8) {
            bytes[8..10].copy_from_slice(&method.to_le_bytes());
            let central = central_offset(&bytes);
            bytes[central + 10..central + 12].copy_from_slice(&method.to_le_bytes());
        }
        bytes
    }

    /// Builds a one-entry archive that stores its content uncompressed.
    pub(crate) fn stored(name: &str, content: &[u8]) -> Vec<u8> {
        archive(name, content, 0)
    }

    /// Builds a one-entry archive that deflates its content.
    pub(crate) fn deflated(name: &str, content: &[u8]) -> Vec<u8> {
        archive(name, content, 8)
    }

    pub(super) fn central_offset(bytes: &[u8]) -> usize {
        let end = bytes.len() - 22;
        usize::try_from(u32::from_le_bytes(
            bytes[end + 16..end + 20].try_into().expect("an offset"),
        ))
        .expect("an archive offset fits usize")
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
                .contains("compression method not supported: 99")
        );
    }

    #[test]
    fn bytes_that_are_not_an_archive_are_refused() {
        assert!(
            parse(b"not an archive at all, and long enough to be one".to_vec())
                .err()
                .expect("this must not parse")
                .contains("Could not find EOCD")
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
                .contains("Invalid CDFH offset in EOCD")
        );
    }

    #[test]
    fn an_entry_whose_bytes_run_past_the_archive_is_refused() {
        let mut bytes = stored("plain.txt", b"stored");
        // Claim a compressed length the archive does not hold.
        bytes[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        let central = super::fixture::central_offset(&bytes);
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

    #[test]
    fn duplicate_central_directory_entries_are_refused() {
        let mut bytes = stored("same.class", b"content");
        let central = super::fixture::central_offset(&bytes);
        let end = bytes.len() - 22;
        let record = bytes[central..end].to_vec();
        bytes.splice(end..end, record.iter().copied());
        let end = bytes.len() - 22;
        bytes[end + 8..end + 10].copy_from_slice(&2_u16.to_le_bytes());
        bytes[end + 10..end + 12].copy_from_slice(&2_u16.to_le_bytes());
        let size = u32::try_from(record.len() * 2).expect("a small directory");
        bytes[end + 12..end + 16].copy_from_slice(&size.to_le_bytes());

        let error = parse(bytes)
            .err()
            .expect("duplicate names must not be collapsed");

        assert!(error.contains("duplicate"), "{error}");
    }

    #[test]
    fn declared_size_caps_deflate_output_while_reading() {
        let mut bytes = deflated("large.class", &vec![42; 32_768]);
        let central = super::fixture::central_offset(&bytes);
        bytes[22..26].copy_from_slice(&1_u32.to_le_bytes());
        bytes[central + 24..central + 28].copy_from_slice(&1_u32.to_le_bytes());
        let archive = parse(bytes).expect("a structurally valid central directory");

        let error = archive
            .entry("large.class")
            .expect_err("the output exceeds its cap");

        assert!(
            error
                .to_string()
                .contains("File is larger than its declared uncompressed size"),
            "{error}"
        );
    }

    #[test]
    fn output_shorter_than_declared_is_refused() {
        let mut bytes = stored("short.class", b"x");
        let central = super::fixture::central_offset(&bytes);
        bytes[22..26].copy_from_slice(&2_u32.to_le_bytes());
        bytes[central + 24..central + 28].copy_from_slice(&2_u32.to_le_bytes());
        let archive = parse(bytes).expect("a structurally valid central directory");

        let error = archive
            .entry("short.class")
            .expect_err("the output is incomplete");

        assert!(error.to_string().contains("declared size"), "{error}");
    }

    #[test]
    fn enumeration_is_sorted_independently_of_archive_order() {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for name in ["z.class", "a.class", "m.class"] {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .expect("an empty fixture entry");
        }
        let bytes = writer.finish().expect("a complete archive").into_inner();
        let archive = parse(bytes).expect("a valid archive");

        assert_eq!(
            archive.names().collect::<Vec<_>>(),
            ["a.class", "m.class", "z.class"]
        );
    }

    /// Adds central metadata without altering the local file or its payload.
    fn with_central_extra(mut bytes: Vec<u8>, extra: &[u8]) -> Vec<u8> {
        let central = super::fixture::central_offset(&bytes);
        assert_eq!(&bytes[central + 30..central + 32], &[0, 0]);
        let name_len = usize::from(u16::from_le_bytes(
            bytes[central + 28..central + 30]
                .try_into()
                .expect("name length"),
        ));
        let at = central + 46 + name_len;
        bytes.splice(at..at, extra.iter().copied());
        bytes[central + 30..central + 32].copy_from_slice(
            &u16::try_from(extra.len())
                .expect("short extra")
                .to_le_bytes(),
        );
        let end = bytes.len() - 22;
        let size = u32::from_le_bytes(bytes[end + 12..end + 16].try_into().expect("central size"));
        let size = size + u32::try_from(extra.len()).expect("short extra");
        bytes[end + 12..end + 16].copy_from_slice(&size.to_le_bytes());
        bytes
    }

    #[test]
    fn unicode_path_metadata_cannot_replace_the_raw_entry_identity() {
        // CRC32("abcdef") is 0x4b8e39ef. zip would otherwise rename it "utf-8".
        let extra = [
            0x75, 0x70, 10, 0, 1, 0xef, 0x39, 0x8e, 0x4b, b'u', b't', b'f', b'-', b'8',
        ];
        let bytes = with_central_extra(stored("abcdef", b"x"), &extra);
        let error = parse(bytes)
            .err()
            .expect("Unicode aliases are not jar entry identities");

        assert!(error.contains("Unicode path extra fields"), "{error}");
    }

    #[test]
    fn zip64_cannot_expand_the_supported_declared_size_limit() {
        let bytes = stored("large.class", b"x");
        let central = super::fixture::central_offset(&bytes);
        let mut extra = vec![1, 0, 16, 0];
        extra.extend((u64::from(u32::MAX) + 1).to_le_bytes());
        extra.extend(1_u64.to_le_bytes());
        let mut bytes = with_central_extra(bytes, &extra);
        bytes[central + 20..central + 28].fill(0xff);
        let error = parse(bytes)
            .err()
            .expect("Zip64 must not lift the output admission cap");

        assert!(error.contains("32-bit uncompressed size"), "{error}");
    }
}
