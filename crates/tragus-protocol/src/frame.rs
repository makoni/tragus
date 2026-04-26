// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

//! Generic AAP frame: `prefix(4) | opcode(1) | reserved(1) | payload(N)`.
//!
//! Every typed payload sits on top of this; pulling the framing out keeps
//! the per-opcode modules small and lets us share length/prefix validation
//! in one place.

use crate::error::ProtocolError;

/// Every regular AAP message starts with this four-byte prefix.
///
/// The handshake packet (see [`crate::HANDSHAKE`]) is the only exception
/// and uses a different prefix entirely.
pub const FRAME_PREFIX: [u8; 4] = [0x04, 0x00, 0x04, 0x00];

/// Length of the fixed AAP frame header (prefix + opcode + reserved).
pub const FRAME_HEADER_LEN: usize = 6;

/// Byte at offset 5 of every frame. Always observed as `0x00` in captures.
const RESERVED: u8 = 0x00;

/// Borrowed view over a single AAP frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame<'a> {
    pub opcode: u8,
    pub payload: &'a [u8],
}

/// Owned counterpart of [`Frame`]. Use this when a frame needs to outlive
/// the byte buffer it was parsed from — e.g. when crossing async boundaries
/// or being put on a channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedFrame {
    pub opcode: u8,
    pub payload: Vec<u8>,
}

impl<'a> From<Frame<'a>> for OwnedFrame {
    fn from(borrowed: Frame<'a>) -> Self {
        Self {
            opcode: borrowed.opcode,
            payload: borrowed.payload.to_vec(),
        }
    }
}

impl<'a> Frame<'a> {
    /// Decode a byte slice from the wire.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < FRAME_HEADER_LEN {
            return Err(ProtocolError::TooShort {
                expected: FRAME_HEADER_LEN,
                got: bytes.len(),
            });
        }
        if bytes[..4] != FRAME_PREFIX {
            return Err(ProtocolError::InvalidPrefix);
        }
        Ok(Frame {
            opcode: bytes[4],
            payload: &bytes[FRAME_HEADER_LEN..],
        })
    }

    /// Build a byte vector ready to write to the L2CAP socket.
    pub fn encode(opcode: u8, payload: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(FRAME_HEADER_LEN + payload.len());
        buf.extend_from_slice(&FRAME_PREFIX);
        buf.push(opcode);
        buf.push(RESERVED);
        buf.extend_from_slice(payload);
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_frame() {
        let bytes = [0x04, 0x00, 0x04, 0x00, 0x4D, 0x00, 0xD7];
        let frame = Frame::parse(&bytes).unwrap();
        assert_eq!(frame.opcode, 0x4D);
        assert_eq!(frame.payload, &[0xD7]);
    }

    #[test]
    fn parse_with_empty_payload() {
        let bytes = [0x04, 0x00, 0x04, 0x00, 0xFF, 0x00];
        let frame = Frame::parse(&bytes).unwrap();
        assert_eq!(frame.opcode, 0xFF);
        assert_eq!(frame.payload, &[] as &[u8]);
    }

    #[test]
    fn parse_too_short() {
        let bytes = [0x04, 0x00, 0x04, 0x00, 0x4D];
        assert_eq!(
            Frame::parse(&bytes),
            Err(ProtocolError::TooShort {
                expected: 6,
                got: 5,
            }),
        );
    }

    #[test]
    fn parse_rejects_wrong_prefix() {
        let bytes = [0x05, 0x00, 0x04, 0x00, 0x4D, 0x00];
        assert_eq!(Frame::parse(&bytes), Err(ProtocolError::InvalidPrefix));
    }

    #[test]
    fn encode_emits_full_frame() {
        let encoded = Frame::encode(0x09, &[0x0D, 0x02]);
        assert_eq!(encoded, [0x04, 0x00, 0x04, 0x00, 0x09, 0x00, 0x0D, 0x02]);
    }

    #[test]
    fn encode_then_parse_roundtrip() {
        let encoded = Frame::encode(0x4D, &[0xD7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let frame = Frame::parse(&encoded).unwrap();
        assert_eq!(frame.opcode, 0x4D);
        assert_eq!(
            frame.payload,
            &[0xD7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        );
    }
}
