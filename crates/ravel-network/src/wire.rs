use crate::WireError;
use crate::contract::Packet;

/// Writes big-endian integers and `u32`-length-prefixed variable fields.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PacketWriter {
    bytes: Vec<u8>,
}

impl PacketWriter {
    /// Appends a 32-bit unsigned integer.
    pub fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    /// Appends a boolean as one byte.
    pub fn bool(&mut self, value: bool) {
        self.bytes.push(u8::from(value));
    }

    /// Appends a length-prefixed UTF-8 string.
    ///
    /// # Panics
    ///
    /// Panics when the text is longer than [`u32::MAX`] bytes.
    pub fn text(&mut self, value: &str) {
        self.bytes_field(value.as_bytes());
    }

    /// Appends a length-prefixed byte string.
    ///
    /// # Panics
    ///
    /// Panics when the field is longer than [`u32::MAX`] bytes.
    pub fn bytes_field(&mut self, value: &[u8]) {
        let length = u32::try_from(value.len()).expect("a packet field shorter than 4 GiB");
        self.u32(length);
        self.bytes.extend_from_slice(value);
    }

    /// Returns the bytes written so far.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// Reads fields from a packet body.
#[derive(Clone, Debug)]
pub struct PacketReader<'bytes> {
    bytes: &'bytes [u8],
    read: usize,
}

impl<'bytes> PacketReader<'bytes> {
    /// Starts reading `bytes`.
    #[must_use]
    pub const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, read: 0 }
    }

    /// Returns how many bytes have not been read.
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.bytes.len() - self.read
    }

    /// Reads a 32-bit unsigned integer.
    ///
    /// # Errors
    ///
    /// Returns [`WireError::Truncated`] when fewer than four bytes remain.
    pub fn u32(&mut self) -> Result<u32, WireError> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Reads a boolean.
    ///
    /// # Errors
    ///
    /// Returns [`WireError::Truncated`] for absent bytes or [`WireError::NotABool`] for an invalid byte.
    pub fn bool(&mut self) -> Result<bool, WireError> {
        match self.take(1)?[0] {
            0 => Ok(false),
            1 => Ok(true),
            byte => Err(WireError::NotABool { byte }),
        }
    }

    /// Reads a length-prefixed UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns [`WireError::Truncated`] when the buffer ends inside the field and
    /// [`WireError::NotUtf8`] when the bytes are not UTF-8.
    pub fn text(&mut self) -> Result<&'bytes str, WireError> {
        let bytes = self.bytes_field()?;
        std::str::from_utf8(bytes).map_err(|_| WireError::NotUtf8)
    }

    /// Reads a length-prefixed byte string.
    ///
    /// # Errors
    ///
    /// Returns [`WireError::Truncated`] when the buffer ends inside the field.
    pub fn bytes_field(&mut self) -> Result<&'bytes [u8], WireError> {
        let length = usize::try_from(self.u32()?).unwrap_or(usize::MAX);
        self.take(length)
    }

    /// Refuses any byte the body did not account for.
    ///
    /// # Errors
    ///
    /// Returns [`WireError::TrailingBytes`] when bytes remain.
    pub const fn finish(self) -> Result<(), WireError> {
        let unread = self.remaining();
        if unread == 0 {
            Ok(())
        } else {
            Err(WireError::TrailingBytes { unread })
        }
    }

    fn take(&mut self, count: usize) -> Result<&'bytes [u8], WireError> {
        let available = self.remaining();
        if count > available {
            return Err(WireError::Truncated {
                needed: count,
                available,
            });
        }
        let taken = &self.bytes[self.read..self.read + count];
        self.read += count;
        Ok(taken)
    }
}

/// Encodes `packet` with its contract's name and version before the body.
pub fn encode<P: Packet>(packet: &P) -> Vec<u8> {
    let contract = P::contract();
    let mut writer = PacketWriter::default();
    writer.text(&contract.id().to_string());
    writer.u32(contract.version());
    packet.write(&mut writer);
    writer.finish()
}

/// Decodes a payload as `P`, refusing anything that is not exactly one `P`.
///
/// # Errors
///
/// Returns [`WireError`] for a payload naming another contract, written under
/// another version, ending early, or ending with bytes left over.
pub fn decode<P: Packet>(bytes: &[u8]) -> Result<P, WireError> {
    let contract = P::contract();
    let mut reader = PacketReader::new(bytes);

    let named = reader.text()?;
    let expected = contract.id().to_string();
    if named != expected {
        return Err(WireError::Identity {
            expected,
            found: named.to_owned(),
        });
    }
    let version = reader.u32()?;
    if version != contract.version() {
        return Err(WireError::Version {
            expected: contract.version(),
            found: version,
        });
    }

    let packet = P::read(&mut reader)?;
    reader.finish()?;
    Ok(packet)
}
