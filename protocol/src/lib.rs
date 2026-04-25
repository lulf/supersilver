#![no_std]

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncoderState {
    pub left: i32,
    pub right: i32,
    pub left_pressed: bool,
    pub right_pressed: bool,
}

/// Encode an EncoderState into a COBS-framed postcard message.
/// Returns the number of bytes written to `buf`.
pub fn encode(state: &EncoderState, buf: &mut [u8]) -> Result<usize, postcard::Error> {
    let bytes = postcard::to_slice_cobs(state, buf)?;
    Ok(bytes.len())
}

/// Accumulates bytes and decodes COBS-framed postcard messages.
pub struct Decoder {
    buf: [u8; 64],
    len: usize,
}

impl Decoder {
    pub const fn new() -> Self {
        Self {
            buf: [0; 64],
            len: 0,
        }
    }

    /// Feed a chunk of bytes. Calls `f` for each decoded message.
    /// Returns Err if a frame fails to decode.
    pub fn feed<F>(&mut self, data: &[u8], mut f: F) -> Result<(), postcard::Error>
    where
        F: FnMut(EncoderState),
    {
        for &byte in data {
            if byte == 0x00 {
                // Sentinel: decode the accumulated frame
                if self.len > 0 {
                    let state =
                        postcard::from_bytes_cobs::<EncoderState>(&mut self.buf[..self.len])?;
                    f(state);
                    self.len = 0;
                }
            } else if self.len < self.buf.len() {
                self.buf[self.len] = byte;
                self.len += 1;
            } else {
                // Overflow: discard frame
                self.len = 0;
                return Err(postcard::Error::SerializeBufferFull);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
extern crate alloc;

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn round_trip_zeros() {
        let state = EncoderState {
            left: 0,
            right: 0,
            left_pressed: false,
            right_pressed: false,
        };
        let mut buf = [0u8; 32];
        let len = encode(&state, &mut buf).unwrap();
        assert!(len > 0);
        assert_eq!(buf[len - 1], 0x00, "COBS frame must end with sentinel");

        let mut decoded = Vec::new();
        let mut decoder = Decoder::new();
        decoder.feed(&buf[..len], |s| decoded.push(s)).unwrap();
        assert_eq!(decoded, vec![state]);
    }

    #[test]
    fn round_trip_positive() {
        let state = EncoderState {
            left: 42,
            right: 100,
            left_pressed: true,
            right_pressed: false,
        };
        let mut buf = [0u8; 32];
        let len = encode(&state, &mut buf).unwrap();

        let mut decoded = Vec::new();
        let mut decoder = Decoder::new();
        decoder.feed(&buf[..len], |s| decoded.push(s)).unwrap();
        assert_eq!(decoded, vec![state]);
    }

    #[test]
    fn round_trip_negative() {
        let state = EncoderState {
            left: -99,
            right: -1,
            left_pressed: false,
            right_pressed: true,
        };
        let mut buf = [0u8; 32];
        let len = encode(&state, &mut buf).unwrap();

        let mut decoded = Vec::new();
        let mut decoder = Decoder::new();
        decoder.feed(&buf[..len], |s| decoded.push(s)).unwrap();
        assert_eq!(decoded, vec![state]);
    }

    #[test]
    fn round_trip_large_values() {
        let state = EncoderState {
            left: i32::MAX,
            right: i32::MIN,
            left_pressed: true,
            right_pressed: true,
        };
        let mut buf = [0u8; 32];
        let len = encode(&state, &mut buf).unwrap();

        let mut decoded = Vec::new();
        let mut decoder = Decoder::new();
        decoder.feed(&buf[..len], |s| decoded.push(s)).unwrap();
        assert_eq!(decoded, vec![state]);
    }

    #[test]
    fn decode_multiple_messages_in_stream() {
        let s1 = EncoderState {
            left: 1,
            right: 2,
            left_pressed: false,
            right_pressed: false,
        };
        let s2 = EncoderState {
            left: -3,
            right: 4,
            left_pressed: true,
            right_pressed: false,
        };

        let mut wire = [0u8; 64];
        let len1 = encode(&s1, &mut wire).unwrap();
        let len2 = encode(&s2, &mut wire[len1..]).unwrap();

        let mut decoded = Vec::new();
        let mut decoder = Decoder::new();
        decoder
            .feed(&wire[..len1 + len2], |s| decoded.push(s))
            .unwrap();
        assert_eq!(decoded, vec![s1, s2]);
    }

    #[test]
    fn decode_split_across_feeds() {
        let state = EncoderState {
            left: 10,
            right: 20,
            left_pressed: false,
            right_pressed: true,
        };
        let mut buf = [0u8; 32];
        let len = encode(&state, &mut buf).unwrap();
        assert!(len > 2, "need at least a few bytes to split");

        let mid = len / 2;
        let mut decoded = Vec::new();
        let mut decoder = Decoder::new();

        // First half: no message yet
        decoder.feed(&buf[..mid], |s| decoded.push(s)).unwrap();
        assert!(decoded.is_empty());

        // Second half: message completes
        decoder.feed(&buf[mid..len], |s| decoded.push(s)).unwrap();
        assert_eq!(decoded, vec![state]);
    }

    #[test]
    fn empty_sentinel_skipped() {
        let mut decoded = Vec::new();
        let mut decoder = Decoder::new();
        // Just sentinels, no data
        decoder.feed(&[0x00, 0x00, 0x00], |s| decoded.push(s)).unwrap();
        assert!(decoded.is_empty());
    }
}
