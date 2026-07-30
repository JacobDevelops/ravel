//! Frame protocol shared by the ADG-2 host and its helper process.
//!
//! One opcode byte plus a little-endian `u64` in each direction. The protocol is
//! deliberately unbatched: it measures what a synchronous per-call boundary
//! costs, which is the question a hook has to answer. Batching would amortize it
//! and is recorded in the ADR as the alternative it is.

#![forbid(unsafe_code)]

use std::io::{self, Read, Write};

/// Run the mod body on the payload.
pub const OPCODE_CALL: u8 = 1;
/// Make a mod-to-mod call, which the mod routes back through the host.
pub const OPCODE_PEER: u8 = 2;
/// End the process inside the mod.
pub const OPCODE_CRASH: u8 = 3;
/// Report the contract version the mod was built against.
pub const OPCODE_ABI_VERSION: u8 = 4;
/// Ask for an entry point this mod does not have.
pub const OPCODE_UNKNOWN_ENTRY: u8 = 5;

/// The frame carries a result.
pub const TAG_RESULT: u8 = 0;
/// The frame asks the host to run a peer call and send its result back.
pub const TAG_PEER_REQUEST: u8 = 1;
/// The frame reports that the request was refused.
pub const TAG_REFUSED: u8 = 2;

/// Bytes in one frame: one tag or opcode plus one little-endian `u64`.
pub const FRAME_BYTES: usize = 9;

/// Writes one frame and flushes it, because the other side is blocked on it.
///
/// # Errors
///
/// Returns an error when the stream cannot take the frame.
pub fn write_frame(stream: &mut impl Write, tag: u8, value: u64) -> io::Result<()> {
    let mut frame = [0_u8; FRAME_BYTES];
    frame[0] = tag;
    frame[1..].copy_from_slice(&value.to_le_bytes());
    stream.write_all(&frame)?;
    stream.flush()
}

/// Reads one frame, or `None` when the other side closed the stream.
///
/// # Errors
///
/// Returns an error when the stream fails for any reason other than a clean end.
pub fn read_frame(stream: &mut impl Read) -> io::Result<Option<(u8, u64)>> {
    let mut frame = [0_u8; FRAME_BYTES];
    match stream.read_exact(&mut frame) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    let value = u64::from_le_bytes(frame[1..].try_into().unwrap_or([0; 8]));
    Ok(Some((frame[0], value)))
}

#[cfg(test)]
mod tests {
    use super::{OPCODE_CALL, read_frame, write_frame};

    #[test]
    fn frames_round_trip_and_end_cleanly() {
        let mut buffer = Vec::new();
        write_frame(&mut buffer, OPCODE_CALL, 7).expect("a vector should take a frame");

        let mut stream = buffer.as_slice();
        assert_eq!(
            read_frame(&mut stream).expect("the frame should read back"),
            Some((OPCODE_CALL, 7))
        );
        assert_eq!(
            read_frame(&mut stream).expect("a closed stream is not an error"),
            None
        );
    }
}
