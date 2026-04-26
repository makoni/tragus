// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

//! `SET_FEATURE_FLAGS` command (AAP opcode `0x4D`).
//!
//! Sent once at handshake time on AirPods Pro 2 to enable
//! conversational-awareness ducking while audio is playing and to enable
//! Adaptive Transparency. Without this packet, conversational awareness
//! still works but only when no audio is playing — see `AAP Definitions.md`.
//!
//! The eight-byte payload is opaque flags. Apple's value is captured as
//! [`SetFeatureFlags::PRO2_DEFAULT`]; bit semantics are not publicly
//! documented.

use crate::error::ProtocolError;

/// AAP opcode for the feature-flags setter.
pub const OPCODE: u8 = 0x4D;

const PAYLOAD_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetFeatureFlags {
    pub flags: [u8; PAYLOAD_LEN],
}

impl SetFeatureFlags {
    /// Captured value Apple sends on Pro 2 — enables CA during audio +
    /// Adaptive Transparency.
    pub const PRO2_DEFAULT: Self = Self {
        flags: [0xD7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    };

    pub fn parse(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < PAYLOAD_LEN {
            return Err(ProtocolError::TooShort {
                expected: PAYLOAD_LEN,
                got: payload.len(),
            });
        }
        let mut flags = [0u8; PAYLOAD_LEN];
        flags.copy_from_slice(&payload[..PAYLOAD_LEN]);
        Ok(Self { flags })
    }

    pub fn encode_payload(&self) -> [u8; PAYLOAD_LEN] {
        self.flags
    }
}

#[cfg(test)]
mod tests {
    use crate::feature_flags::{OPCODE, SetFeatureFlags};
    use crate::frame::Frame;

    use crate::error::ProtocolError;

    /// Captured by Apple's stack on Pro 2: enables conversational awareness
    /// during audio playback and Adaptive Transparency. Reproduced from
    /// `AAP Definitions.md`.
    #[test]
    fn pro2_default_packet_round_trips() {
        let bytes = [
            0x04, 0x00, 0x04, 0x00, 0x4D, 0x00, // header (opcode 0x4D)
            0xD7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let frame = Frame::parse(&bytes).unwrap();
        assert_eq!(frame.opcode, OPCODE);

        let cmd = SetFeatureFlags::parse(frame.payload).unwrap();
        assert_eq!(cmd, SetFeatureFlags::PRO2_DEFAULT);
        assert_eq!(
            cmd.encode_payload(),
            [0xD7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        );
    }

    #[test]
    fn payload_shorter_than_eight_bytes_is_rejected() {
        let payload = [0xD7, 0x00, 0x00];
        assert_eq!(
            SetFeatureFlags::parse(&payload),
            Err(ProtocolError::TooShort {
                expected: 8,
                got: 3,
            }),
        );
    }
}
