// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Apple Continuity Proximity-Pairing message (BLE manuf data 0x004C).
//!
//! Format from `Proximity Pairing Message.md` and the LibrePods Android
//! `BLEManager.kt`:
//!
//! ```text
//! [0]    0x07         — proximity-pairing prefix
//! [1]    0x12         — length (18 bytes)
//! [2]    {0|1}        — pairing mode (1 = paired)
//! [3..5] u16 BE       — model ID (e.g. 0x1420 = AirPods Pro 2)
//! [5]    bitfield     — pod-status flags
//! [6]    nibbles      — primary pod (high) + other pod (low) battery
//! [7]    nibbles      — case battery (high) + flags (low)
//! [8]    bitfield     — bit 3 = lid closed (0 = open)
//! [9]    color        — colour code
//! [10]   conn state
//! [11..27] 16 bytes   — encrypted tail (AES-128 ECB with ENC_KEY)
//! ```
//!
//! Battery levels in nibbles use Apple's coarse scheme:
//!
//! ```text
//! 0x0..=0x9 → 0..=90 percent (× 10)
//! 0xA..=0xE → 100 percent
//! 0xF       → unavailable
//! ```
//!
//! AES decryption of the 16-byte tail isn't done here — the caller has
//! the ENC_KEY (from AAP opcode 0x30/0x31) and runs `crate::crypto`
//! against the sliced bytes.

use crate::error::ProtocolError;

/// Length of the visible header before the encrypted tail.
const HEADER_LEN: usize = 27;

/// Prefix byte that identifies a proximity-pairing message.
pub const PROXIMITY_PREFIX: u8 = 0x07;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryLevel {
    Unavailable,
    /// 0..=100 inclusive.
    Percent(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProximityAdvertisement {
    pub paired: bool,
    pub model_id: u16,
    pub status: u8,
    pub case_open: bool,
}

impl ProximityAdvertisement {
    pub fn parse(manuf_data: &[u8]) -> Result<Self, ProtocolError> {
        if manuf_data.len() < HEADER_LEN {
            return Err(ProtocolError::TooShort {
                expected: HEADER_LEN,
                got: manuf_data.len(),
            });
        }
        if manuf_data[0] != PROXIMITY_PREFIX {
            return Err(ProtocolError::InvalidPrefix);
        }
        Ok(Self {
            paired: manuf_data[2] == 0x01,
            model_id: u16::from_be_bytes([manuf_data[3], manuf_data[4]]),
            status: manuf_data[5],
            case_open: (manuf_data[8] & 0x08) == 0,
        })
    }
}

/// Decode one battery nibble (the low 4 bits of the byte) per Apple's
/// proximity-pairing scheme.
pub fn decode_battery_nibble(byte: u8) -> BatteryLevel {
    match byte & 0x0F {
        0xF => BatteryLevel::Unavailable,
        v @ 0x0..=0x9 => BatteryLevel::Percent(v * 10),
        // 0xA..=0xE all round up to full per Apple's encoding.
        _ => BatteryLevel::Percent(100),
    }
}

#[cfg(test)]
mod tests {
    use crate::error::ProtocolError;
    use crate::proximity::{BatteryLevel, ProximityAdvertisement, decode_battery_nibble};

    /// Minimal happy-path: prefix 0x07, length 0x12, paired=1, model
    /// AirPods Pro 2 (0x1420), padding zeros to fill the 27-byte header.
    #[test]
    fn parses_minimal_paired_advertisement() {
        let mut bytes = vec![0u8; 27];
        bytes[0] = 0x07;
        bytes[1] = 0x12;
        bytes[2] = 0x01; // paired
        bytes[3] = 0x14;
        bytes[4] = 0x20;
        bytes[8] = 0x00; // lid bit clear → case open

        let adv = ProximityAdvertisement::parse(&bytes).unwrap();
        assert!(adv.paired);
        assert_eq!(adv.model_id, 0x1420);
        assert!(adv.case_open);
    }

    #[test]
    fn parses_unpaired_advertisement() {
        let mut bytes = vec![0u8; 27];
        bytes[0] = 0x07;
        bytes[1] = 0x12;
        bytes[2] = 0x00;
        bytes[3] = 0x0E;
        bytes[4] = 0x20;
        bytes[8] = 0x08; // lid bit set → case closed

        let adv = ProximityAdvertisement::parse(&bytes).unwrap();
        assert!(!adv.paired);
        assert_eq!(adv.model_id, 0x0E20);
        assert!(!adv.case_open);
    }

    #[test]
    fn wrong_prefix_is_invalid() {
        let mut bytes = vec![0u8; 27];
        bytes[0] = 0xFF;
        assert!(matches!(
            ProximityAdvertisement::parse(&bytes),
            Err(ProtocolError::InvalidPrefix),
        ));
    }

    #[test]
    fn too_short_advertisement() {
        let bytes = vec![0u8; 5];
        assert!(matches!(
            ProximityAdvertisement::parse(&bytes),
            Err(ProtocolError::TooShort { .. }),
        ));
    }

    #[test]
    fn battery_nibble_zero_is_zero_percent() {
        assert_eq!(decode_battery_nibble(0x00), BatteryLevel::Percent(0));
    }

    #[test]
    fn battery_nibble_one_through_nine_scales_by_ten() {
        assert_eq!(decode_battery_nibble(0x05), BatteryLevel::Percent(50));
        assert_eq!(decode_battery_nibble(0x09), BatteryLevel::Percent(90));
    }

    #[test]
    fn battery_nibble_a_through_e_is_full() {
        assert_eq!(decode_battery_nibble(0x0A), BatteryLevel::Percent(100));
        assert_eq!(decode_battery_nibble(0x0E), BatteryLevel::Percent(100));
    }

    #[test]
    fn battery_nibble_f_is_unavailable() {
        assert_eq!(decode_battery_nibble(0x0F), BatteryLevel::Unavailable);
    }

    #[test]
    fn battery_nibble_ignores_high_nibble() {
        // Only the low nibble matters — the upper nibble carries a
        // different component.
        assert_eq!(decode_battery_nibble(0xA5), BatteryLevel::Percent(50));
    }
}
