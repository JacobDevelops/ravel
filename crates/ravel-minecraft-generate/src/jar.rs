//! Read-only JAR access, bounded decompression, and pinned archive metadata contracts.

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

use serde::Deserialize;

use super::GenerateError;
use super::inputs::{Adapter, Spelling};

/// Checks the maximum bytecode release against the adapter's declared source release.
pub(crate) fn verify_legacy_class_release(
    archive: &Archive,
    adapter: &Adapter,
) -> Result<(), GenerateError> {
    let mut maximum = None;
    for name in archive.names().filter(|name| {
        Path::new(name)
            .extension()
            .is_some_and(|ext| ext == "class")
    }) {
        let bytes = archive.entry_prefix::<8>(name)?;
        if bytes[..4] != [0xca, 0xfe, 0xba, 0xbe] {
            return Err(GenerateError::Extraction(format!(
                "{name} has no valid class-file version header"
            )));
        }
        let major = u16::from_be_bytes([bytes[6], bytes[7]]);
        maximum = Some(maximum.map_or(major, |previous: u16| previous.max(major)));
    }
    let release = adapter.effective_source_java_release();
    let expected = release.checked_add(44).ok_or_else(|| {
        GenerateError::Input(format!(
            "sourceJavaRelease {release} has no class-file major"
        ))
    })?;
    if maximum != Some(expected) {
        return Err(GenerateError::Drift(format!(
            "sourceJavaRelease {release} requires maximum class-file major {expected}; \
             pinned client jar states {maximum:?}"
        )));
    }
    Ok(())
}
/// Checks the declared pack formats against the pinned jar.
///
/// # Errors
///
/// Returns an error if the jar's pack formats are missing or inconsistent with the declaration.
pub(crate) fn verify_pack_format(
    adapter: &Adapter,
    archive: &Archive,
) -> Result<(), GenerateError> {
    if adapter.is_legacy() {
        return Ok(());
    }
    let text = jar_text(archive, JAR_VERSION)?;
    let declared = adapter.pack_format;
    let stated = pack_versions(&text).map_err(|error| {
        GenerateError::Input(format!(
            "cannot read pack_version from {JAR_VERSION} in the pinned jar: {error}"
        ))
    })?;
    let expected = (
        declared.resource,
        declared.resource_minor,
        declared.data,
        declared.data_minor,
    );
    if stated != expected {
        return Err(GenerateError::Drift(format!(
            "the adapter inputs declare packFormat {expected:?} but {JAR_VERSION} in the pinned \
             jar states {stated:?}, as resource major, resource minor, data major, data minor"
        )));
    }
    verify_spelling(declared.spelling, archive)
}

/// Checks the declared metadata spelling against a pack in the pinned jar.
///
/// # Errors
///
/// Returns an error if the jar contains no pack or its metadata uses another spelling.
fn verify_spelling(spelling: Spelling, archive: &Archive) -> Result<(), GenerateError> {
    let Some(name) = archive.names().find(|name| name.ends_with(PACK_METADATA)) else {
        return Err(GenerateError::Input(format!(
            "the pinned jar ships no {PACK_METADATA} of its own, so nothing says which shape a \
             pack this version reads is written in"
        )));
    };
    let name = name.to_owned();
    let text = jar_text(archive, &name)?;
    if !text.contains(spelling.key()) {
        return Err(GenerateError::Drift(format!(
            "the adapter inputs declare packFormat spelling {} but {name} in the pinned jar \
             carries no {}",
            spelling.as_str(),
            spelling.key()
        )));
    }
    Ok(())
}

/// One named entry of the pinned jar, as text.
fn jar_text(archive: &Archive, name: &str) -> Result<String, GenerateError> {
    let bytes = archive.entry(name)?;
    String::from_utf8(bytes).map_err(|error| {
        GenerateError::Input(format!("{name} in the pinned jar is not UTF-8: {error}"))
    })
}

/// The document every pack, Ravel's included, declares its format in.
const PACK_METADATA: &str = "pack.mcmeta";

/// The document inside a client jar that states what the build is.
const JAR_VERSION: &str = "version.json";

/// The resource and data pack formats one jar's `version.json` states.
fn pack_versions(text: &str) -> Result<(u16, u16, u16, u16), serde_json::Error> {
    #[derive(Deserialize)]
    struct Version {
        pack_version: PackVersion,
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum PackVersion {
        Joined {
            resource: u16,
            data: u16,
        },
        Split {
            resource_major: u16,
            resource_minor: u16,
            data_major: u16,
            data_minor: u16,
        },
    }
    let version: Version = serde_json::from_str(text)?;
    Ok(match version.pack_version {
        PackVersion::Joined { resource, data } => (resource, 0, data, 0),
        PackVersion::Split {
            resource_major,
            resource_minor,
            data_major,
            data_minor,
        } => (resource_major, resource_minor, data_major, data_minor),
    })
}

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
        let bytes = fs::read(path).map_err(|error| GenerateError::io(&name, error))?;
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

    /// Reads a fixed-size header without allocating or inflating the whole entry.
    ///
    /// This deliberately does not validate the unread body or its CRC.
    pub(super) fn entry_prefix<const N: usize>(
        &self,
        name: &str,
    ) -> Result<[u8; N], GenerateError> {
        self.read_entry_prefix(name)
            .map_err(|detail| GenerateError::Extraction(format!("{name}: {detail}")))
    }

    fn read_entry_prefix<const N: usize>(&self, name: &str) -> Result<[u8; N], String> {
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
        let mut bytes = [0; N];
        file.read_exact(&mut bytes)
            .map_err(|error| error.to_string())?;
        Ok(bytes)
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
        let mut bytes = entries(&[(name, content)], method);
        if !matches!(method, 0 | 8) {
            bytes[8..10].copy_from_slice(&method.to_le_bytes());
            let central = central_offset(&bytes);
            bytes[central + 10..central + 12].copy_from_slice(&method.to_le_bytes());
        }
        bytes
    }

    /// Builds an archive with multiple entries, storing or deflating their contents.
    pub(crate) fn entries(entries: &[(&str, &[u8])], method: u16) -> Vec<u8> {
        let compression = if method == 8 {
            CompressionMethod::Deflated
        } else {
            CompressionMethod::Stored
        };
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for &(name, content) in entries {
            writer
                .start_file(
                    name,
                    SimpleFileOptions::default().compression_method(compression),
                )
                .expect("a valid fixture entry");
            writer.write_all(content).expect("writing to memory");
        }
        writer.finish().expect("a complete archive").into_inner()
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
    fn entry_prefix_reads_only_the_requested_header() {
        for method in [0, 8] {
            let content = b"header!!body".repeat(4096);
            let mut bytes = archive("Game.class", &content, method);
            let central = super::fixture::central_offset(&bytes);
            // An unread body must not be consumed just to validate its CRC.
            bytes[14..18].fill(0);
            bytes[central + 16..central + 20].fill(0);
            let archive = parse(bytes).expect("an archive");

            assert_eq!(
                archive.entry_prefix::<8>("Game.class").expect("a header"),
                *b"header!!"
            );
            assert!(archive.entry("Game.class").is_err());
        }
    }

    #[test]
    fn entry_prefix_rejects_missing_and_short_entries() {
        let archive = parse(stored("short.class", b"short")).expect("an archive");

        assert!(archive.entry_prefix::<8>("missing.class").is_err());
        assert!(archive.entry_prefix::<8>("short.class").is_err());
    }

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
        assert!(archive.entry_prefix::<8>("plain.txt").is_err());

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
        let archive = parse(super::fixture::entries(
            &[("z.class", b"z"), ("a.class", b"a"), ("m.class", b"m")],
            0,
        ))
        .expect("a valid archive");

        assert_eq!(
            archive.names().collect::<Vec<_>>(),
            ["a.class", "m.class", "z.class"]
        );
        assert_eq!(archive.entry("a.class").expect("an entry"), b"a");
        assert_eq!(archive.entry("m.class").expect("an entry"), b"m");
        assert_eq!(archive.entry("z.class").expect("an entry"), b"z");
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

#[cfg(test)]
mod metadata_tests {
    use super as jar;
    use super::Adapter;
    use crate::test_fixtures::{document, legacy_adapter};

    #[test]
    fn both_spellings_of_a_jars_pack_version_are_read_and_neither_reads_the_other() {
        let joined = r#"{"data": 9, "pack_version": {"resource": 34, "data": 48}}"#;
        assert_eq!(
            super::pack_versions(joined).expect("joined formats"),
            (34, 0, 48, 0)
        );

        let split = r#"{"pack_version": {"resource_major": 84, "resource_minor": 0,
          "data_major": 101, "data_minor": 1}, "world_version": 4790}"#;
        assert_eq!(
            super::pack_versions(split).expect("split formats"),
            (84, 0, 101, 1)
        );

        assert!(super::pack_versions(r#"{"world_version": 4790}"#).is_err());
        assert!(super::pack_versions(r#"{"pack_version": {"data": 48}}"#).is_err());
    }
    #[test]
    fn only_explicit_legacy_capability_accepts_a_jar_without_pack_version_metadata() {
        let archive = jar::fixture::parse(jar::fixture::stored("unused", b""))
            .expect("an archive without version metadata");
        super::verify_pack_format(&legacy_adapter(), &archive)
            .expect("legacy pack constants do not pretend to come from jar metadata");
        let modern: Adapter =
            toml::from_str(&document("unobfuscated-class-files", "")).expect("a modern adapter");
        super::verify_pack_format(&modern, &archive)
            .expect_err("modern jars must declare their pack formats");
    }

    #[test]
    fn legacy_bytecode_maximum_follows_the_declared_source_release() {
        for release in [8_u16, 17] {
            let mut adapter = legacy_adapter();
            adapter.source_java_release = Some(release);
            for (major, accepted) in [
                (release + 43, false),
                (release + 44, true),
                (release + 45, false),
            ] {
                let mut header = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0];
                header.extend(major.to_be_bytes());
                let archive = jar::fixture::parse(jar::fixture::stored("Game.class", &header))
                    .expect("class-version fixture");
                assert_eq!(
                    super::verify_legacy_class_release(&archive, &adapter).is_ok(),
                    accepted
                );
            }
        }
    }

    #[test]
    fn pack_numbers_must_be_valid_json_at_the_declared_path() {
        let nested = r#"{"pack_version":{"note":{"resource":1},"data":48,"resource":34}}"#;
        assert_eq!(
            super::pack_versions(nested).expect("valid pack versions"),
            (34, 0, 48, 0)
        );
        for value in ["21.5", "-21", "65536", "\"21\"", "21garbage"] {
            let packs = format!(r#"{{"pack_version":{{"resource":{value},"data":48}}}}"#);
            assert!(super::pack_versions(&packs).is_err(), "{packs}");
        }
    }

    #[test]
    fn pack_metadata_parse_failures_keep_the_json_diagnostic() {
        let adapter: Adapter =
            toml::from_str(&document("unobfuscated-class-files", "")).expect("modern adapter");
        for text in [
            "{",
            "{}",
            r#"{"pack_version":{"resource":65536,"data":48}}"#,
        ] {
            let archive =
                jar::fixture::parse(jar::fixture::stored("version.json", text.as_bytes()))
                    .expect("version metadata archive");
            let error =
                super::verify_pack_format(&adapter, &archive).expect_err("invalid metadata");
            let message = error.to_string();
            assert!(
                message.contains("cannot read pack_version from version.json"),
                "{message}"
            );
            assert!(!message.contains("states no pack_version"), "{message}");
        }
    }
}
