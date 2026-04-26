// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

//! `REQUEST_NOTIFICATIONS` (AAP opcode `0x0F`).
//!
//! Sent once after handshake to subscribe to battery / ear-detection /
//! ANC mode / conv. awareness pushes from the AirPods. Without this, the
//! AirPods never push their state.
//!
//! The four-byte payload is a flag mask. Apple's captured values are
//! `FF FF FF FF` (everything) and `FF FF FE FF` (everything except ear
//! detection); per-bit semantics are not publicly documented.

use crate::error::ProtocolError;

/// AAP opcode for `REQUEST_NOTIFICATIONS`.
pub const OPCODE: u8 = 0x0F;

const PAYLOAD_LEN: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationFlags(pub [u8; PAYLOAD_LEN]);

impl NotificationFlags {
    /// Subscribe to every push the AirPods can send.
    pub const ALL: Self = Self([0xFF, 0xFF, 0xFF, 0xFF]);

    /// Same as [`ALL`] but with the ear-detection bit cleared. Used when
    /// the upper layers want to handle in-ear via the BLE advertisement
    /// instead of via opcode 0x06.
    pub const ALL_EXCEPT_EAR_DETECTION: Self = Self([0xFF, 0xFF, 0xFE, 0xFF]);

    pub fn parse(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < PAYLOAD_LEN {
            return Err(ProtocolError::TooShort {
                expected: PAYLOAD_LEN,
                got: payload.len(),
            });
        }
        let mut bytes = [0u8; PAYLOAD_LEN];
        bytes.copy_from_slice(&payload[..PAYLOAD_LEN]);
        Ok(Self(bytes))
    }

    pub fn encode_payload(&self) -> [u8; PAYLOAD_LEN] {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::error::ProtocolError;
    use crate::frame::Frame;
    use crate::notifications::{NotificationFlags, OPCODE};

    /// Both flag patterns the AAP spec captures verbatim:
    ///   04 00 04 00 0F 00 FF FF FF FF   — subscribe to everything
    ///   04 00 04 00 0F 00 FF FF FE FF   — same minus ear detection
    #[test]
    fn all_flags_round_trips_against_spec() {
        let payload = NotificationFlags::ALL.encode_payload();
        assert_eq!(payload, [0xFF, 0xFF, 0xFF, 0xFF]);

        let full = Frame::encode(OPCODE, &payload);
        assert_eq!(
            full,
            [0x04, 0x00, 0x04, 0x00, 0x0F, 0x00, 0xFF, 0xFF, 0xFF, 0xFF],
        );

        let frame = Frame::parse(&full).unwrap();
        assert_eq!(frame.opcode, OPCODE);
        assert_eq!(
            NotificationFlags::parse(frame.payload).unwrap(),
            NotificationFlags::ALL,
        );
    }

    #[test]
    fn all_except_ear_detection_round_trips() {
        let payload = NotificationFlags::ALL_EXCEPT_EAR_DETECTION.encode_payload();
        assert_eq!(payload, [0xFF, 0xFF, 0xFE, 0xFF]);
    }

    #[test]
    fn payload_shorter_than_four_bytes_is_rejected() {
        assert_eq!(
            NotificationFlags::parse(&[0xFF, 0xFF]),
            Err(ProtocolError::TooShort {
                expected: 4,
                got: 2,
            }),
        );
    }
}
