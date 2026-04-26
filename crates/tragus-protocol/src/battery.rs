// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

//! Battery status notifications (AAP opcode `0x04`).
//!
//! Wire format of the payload:
//!
//! ```text
//! count : u8
//! repeated count times:
//!     component : u8   // see [`BatteryComponent`]
//!     0x01             // spacer
//!     level     : u8   // 0..=100 percent
//!     status    : u8   // see [`BatteryStatus`]
//!     0x01             // spacer
//! ```
//!
//! Note: the per-byte interpretation table in `AAP Definitions.md`
//! mislabels the first entry of the example packet as "Left" — the
//! component byte there is `0x02`, which is **Right** according to the
//! component table in the same document and to LibrePods' Android code.
//! We follow the table.

use crate::error::ProtocolError;

/// AAP opcode used for battery notifications.
pub const OPCODE: u8 = 0x04;

const ENTRY_LEN: usize = 5;
const SPACER: u8 = 0x01;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryComponent {
    Left = 0x04,
    Right = 0x02,
    Case = 0x08,
}

impl BatteryComponent {
    fn from_byte(b: u8) -> Result<Self, ProtocolError> {
        match b {
            0x04 => Ok(Self::Left),
            0x02 => Ok(Self::Right),
            0x08 => Ok(Self::Case),
            other => Err(ProtocolError::UnknownBatteryComponent(other)),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryStatus {
    Unknown = 0x00,
    Charging = 0x01,
    Discharging = 0x02,
    Disconnected = 0x04,
}

impl BatteryStatus {
    fn from_byte(b: u8) -> Result<Self, ProtocolError> {
        match b {
            0x00 => Ok(Self::Unknown),
            0x01 => Ok(Self::Charging),
            0x02 => Ok(Self::Discharging),
            0x04 => Ok(Self::Disconnected),
            other => Err(ProtocolError::UnknownBatteryStatus(other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryEntry {
    pub component: BatteryComponent,
    /// Charge level in percent, `0..=100`.
    pub level: u8,
    pub status: BatteryStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryNotification {
    entries: Vec<BatteryEntry>,
}

impl BatteryNotification {
    pub fn new(entries: Vec<BatteryEntry>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[BatteryEntry] {
        &self.entries
    }

    /// Decode the payload that follows an AAP frame header with opcode
    /// [`OPCODE`].
    pub fn parse(payload: &[u8]) -> Result<Self, ProtocolError> {
        let Some(&count_byte) = payload.first() else {
            return Err(ProtocolError::TooShort {
                expected: 1,
                got: 0,
            });
        };
        let count = count_byte as usize;
        let expected = 1 + count * ENTRY_LEN;
        if payload.len() < expected {
            return Err(ProtocolError::TooShort {
                expected,
                got: payload.len(),
            });
        }

        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let off = 1 + i * ENTRY_LEN;
            let component = BatteryComponent::from_byte(payload[off])?;
            check_spacer(payload[off + 1], off + 1)?;
            let level = payload[off + 2];
            let status = BatteryStatus::from_byte(payload[off + 3])?;
            check_spacer(payload[off + 4], off + 4)?;
            entries.push(BatteryEntry {
                component,
                level,
                status,
            });
        }
        Ok(Self { entries })
    }

    /// Encode just the payload — without the AAP frame header. Wrap the
    /// result with [`crate::frame::Frame::encode`] before sending.
    pub fn encode_payload(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + self.entries.len() * ENTRY_LEN);
        // The protocol restricts entries to Left/Right/Case, so `len` fits in u8.
        buf.push(self.entries.len() as u8);
        for entry in &self.entries {
            buf.push(entry.component as u8);
            buf.push(SPACER);
            buf.push(entry.level);
            buf.push(entry.status as u8);
            buf.push(SPACER);
        }
        buf
    }
}

fn check_spacer(byte: u8, offset: usize) -> Result<(), ProtocolError> {
    if byte == SPACER {
        Ok(())
    } else {
        Err(ProtocolError::UnexpectedByte {
            offset,
            expected: SPACER,
            got: byte,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Frame;

    /// Reference packet from `AAP Definitions.md`, captured from AirPods Pro 2.
    const REFERENCE_FRAME: &[u8] = &[
        0x04, 0x00, 0x04, 0x00, 0x04, 0x00, // header (opcode 0x04)
        0x03, // 3 entries
        0x02, 0x01, 0x64, 0x02, 0x01, // Right, 100%, Discharging
        0x04, 0x01, 0x63, 0x01, 0x01, // Left,  99%, Charging
        0x08, 0x01, 0x11, 0x02, 0x01, // Case,  17%, Discharging
    ];

    #[test]
    fn parses_reference_packet() {
        let frame = Frame::parse(REFERENCE_FRAME).unwrap();
        assert_eq!(frame.opcode, OPCODE);

        let notif = BatteryNotification::parse(frame.payload).unwrap();
        assert_eq!(
            notif.entries(),
            &[
                BatteryEntry {
                    component: BatteryComponent::Right,
                    level: 100,
                    status: BatteryStatus::Discharging,
                },
                BatteryEntry {
                    component: BatteryComponent::Left,
                    level: 99,
                    status: BatteryStatus::Charging,
                },
                BatteryEntry {
                    component: BatteryComponent::Case,
                    level: 17,
                    status: BatteryStatus::Discharging,
                },
            ],
        );
    }

    #[test]
    fn encode_then_parse_roundtrip() {
        let original = BatteryNotification::new(vec![
            BatteryEntry {
                component: BatteryComponent::Left,
                level: 50,
                status: BatteryStatus::Discharging,
            },
            BatteryEntry {
                component: BatteryComponent::Right,
                level: 60,
                status: BatteryStatus::Charging,
            },
            BatteryEntry {
                component: BatteryComponent::Case,
                level: 100,
                status: BatteryStatus::Disconnected,
            },
        ]);
        let encoded = original.encode_payload();
        let parsed = BatteryNotification::parse(&encoded).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn empty_payload_is_too_short() {
        assert_eq!(
            BatteryNotification::parse(&[]),
            Err(ProtocolError::TooShort {
                expected: 1,
                got: 0,
            }),
        );
    }

    #[test]
    fn truncated_payload() {
        // Claims two entries but only one fits.
        let payload = [0x02, 0x04, 0x01, 0x50, 0x02, 0x01];
        assert_eq!(
            BatteryNotification::parse(&payload),
            Err(ProtocolError::TooShort {
                expected: 11,
                got: 6,
            }),
        );
    }

    #[test]
    fn unknown_component_byte() {
        let payload = [0x01, 0xAA, 0x01, 0x50, 0x02, 0x01];
        assert_eq!(
            BatteryNotification::parse(&payload),
            Err(ProtocolError::UnknownBatteryComponent(0xAA)),
        );
    }

    #[test]
    fn unknown_status_byte() {
        let payload = [0x01, 0x04, 0x01, 0x50, 0xFF, 0x01];
        assert_eq!(
            BatteryNotification::parse(&payload),
            Err(ProtocolError::UnknownBatteryStatus(0xFF)),
        );
    }

    #[test]
    fn missing_spacer_in_entry() {
        let payload = [0x01, 0x04, 0x99, 0x50, 0x02, 0x01];
        assert_eq!(
            BatteryNotification::parse(&payload),
            Err(ProtocolError::UnexpectedByte {
                offset: 2,
                expected: 0x01,
                got: 0x99,
            }),
        );
    }

    #[test]
    fn zero_entries_parses_to_empty() {
        let notif = BatteryNotification::parse(&[0x00]).unwrap();
        assert!(notif.entries().is_empty());
    }
}
