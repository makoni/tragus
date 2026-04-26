// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

//! Ear detection notifications (AAP opcode `0x06`).
//!
//! Wire format of the payload:
//!
//! ```text
//! primary   : u8   // see [`EarStatus`]
//! secondary : u8
//! ```
//!
//! Per `AAP Definitions.md`: when the primary pod is removed, the AirPods
//! swap roles internally — they re-send a fresh packet with the previously
//! "secondary" pod as the new primary. So upper layers should track the
//! pair as a whole rather than relying on which slot is "primary" right
//! now.

use crate::error::ProtocolError;

/// AAP opcode for ear-detection notifications.
pub const OPCODE: u8 = 0x06;

const PAYLOAD_LEN: usize = 2;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarStatus {
    InEar = 0x00,
    OutOfEar = 0x01,
    InCase = 0x02,
}

impl EarStatus {
    fn from_byte(b: u8) -> Result<Self, ProtocolError> {
        match b {
            0x00 => Ok(Self::InEar),
            0x01 => Ok(Self::OutOfEar),
            0x02 => Ok(Self::InCase),
            other => Err(ProtocolError::UnknownEarStatus(other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EarDetectionNotification {
    pub primary: EarStatus,
    pub secondary: EarStatus,
}

impl EarDetectionNotification {
    pub fn parse(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < PAYLOAD_LEN {
            return Err(ProtocolError::TooShort {
                expected: PAYLOAD_LEN,
                got: payload.len(),
            });
        }
        Ok(Self {
            primary: EarStatus::from_byte(payload[0])?,
            secondary: EarStatus::from_byte(payload[1])?,
        })
    }

    pub fn encode_payload(&self) -> [u8; PAYLOAD_LEN] {
        [self.primary as u8, self.secondary as u8]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Frame;

    #[test]
    fn parses_full_frame() {
        let bytes = [0x04, 0x00, 0x04, 0x00, 0x06, 0x00, 0x00, 0x01];
        let frame = Frame::parse(&bytes).unwrap();
        assert_eq!(frame.opcode, OPCODE);

        let n = EarDetectionNotification::parse(frame.payload).unwrap();
        assert_eq!(n.primary, EarStatus::InEar);
        assert_eq!(n.secondary, EarStatus::OutOfEar);
    }

    #[test]
    fn both_in_case() {
        let n = EarDetectionNotification::parse(&[0x02, 0x02]).unwrap();
        assert_eq!(n.primary, EarStatus::InCase);
        assert_eq!(n.secondary, EarStatus::InCase);
    }

    #[test]
    fn unknown_status_byte() {
        assert_eq!(
            EarDetectionNotification::parse(&[0x00, 0xAA]),
            Err(ProtocolError::UnknownEarStatus(0xAA)),
        );
    }

    #[test]
    fn payload_too_short() {
        assert_eq!(
            EarDetectionNotification::parse(&[0x00]),
            Err(ProtocolError::TooShort {
                expected: 2,
                got: 1,
            }),
        );
    }

    #[test]
    fn encode_roundtrip() {
        let original = EarDetectionNotification {
            primary: EarStatus::InEar,
            secondary: EarStatus::OutOfEar,
        };
        let encoded = original.encode_payload();
        assert_eq!(encoded, [0x00, 0x01]);
        assert_eq!(EarDetectionNotification::parse(&encoded).unwrap(), original);
    }
}
