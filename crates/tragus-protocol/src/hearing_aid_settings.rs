// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

//! Hearing-Aid payload codec (GATT handle `0x002A`).
//!
//! 104-byte little-endian struct, layout from the LibrePods Android
//! `HearingAid.kt`:
//!
//! ```text
//! offset  field                                bytes  type
//! ------  -----                                -----  -----
//! 0       header                               4      [0x02 0x02 0x60 0x00]
//! 4–35    left  EQ (8 bands)                   32     [f32; 8]
//! 36      left  amplification                  4      f32  (-1.0..=+1.0)
//! 40      left  tone                           4      f32
//! 44      left  conversation boost             4      f32  (>0.5 == on)
//! 48      left  ambient noise reduction        4      f32  (0.0..=1.0)
//! 52–83   right EQ (8 bands)                   32     [f32; 8]
//! 84      right amplification                  4      f32
//! 88      right tone                           4      f32
//! 92      right conversation boost             4      f32
//! 96      right ambient noise reduction        4      f32
//! 100     own voice amplification              4      f32  (mandatory)
//! ```
//!
//! Per-ear sub-layout shared with [`crate::transparency`] via
//! [`crate::channel`].

use crate::channel::{ChannelSettings, encode_channel, parse_channel, read_f32, write_f32};
use crate::error::ProtocolError;

/// GATT characteristic handle for Hearing-Aid.
pub const HANDLE: u16 = 0x002A;

const PAYLOAD_LEN: usize = 104;

/// Magic header Apple's stack always emits. The bytes are reserved
/// per spec, we round-trip them verbatim.
const HEADER: [u8; 4] = [0x02, 0x02, 0x60, 0x00];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HearingAidSettings {
    pub left: ChannelSettings,
    pub right: ChannelSettings,
    pub own_voice_amplification: f32,
}

impl HearingAidSettings {
    pub fn parse(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < PAYLOAD_LEN {
            return Err(ProtocolError::TooShort {
                expected: PAYLOAD_LEN,
                got: bytes.len(),
            });
        }
        Ok(Self {
            left: parse_channel(&bytes[4..52])?,
            right: parse_channel(&bytes[52..100])?,
            own_voice_amplification: read_f32(bytes, 100)?,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(PAYLOAD_LEN);
        buf.extend_from_slice(&HEADER);
        encode_channel(&mut buf, &self.left);
        encode_channel(&mut buf, &self.right);
        write_f32(&mut buf, self.own_voice_amplification);
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::EqBands;

    #[test]
    fn att_handle_matches_spec() {
        assert_eq!(HANDLE, 0x002A);
    }

    fn sample() -> HearingAidSettings {
        HearingAidSettings {
            left: ChannelSettings {
                eq: EqBands {
                    bands: [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
                },
                amplification: 0.5,
                tone: 0.5,
                conversation_boost: 1.0,
                ambient_noise_reduction: 0.3,
            },
            right: ChannelSettings {
                eq: EqBands {
                    bands: [-0.1, -0.2, -0.3, -0.4, -0.5, -0.6, -0.7, -0.8],
                },
                amplification: -0.5,
                tone: -0.2,
                conversation_boost: 0.0,
                ambient_noise_reduction: 0.7,
            },
            own_voice_amplification: 0.4,
        }
    }

    #[test]
    fn encode_yields_exact_104_bytes_with_magic_header() {
        let bytes = sample().encode();
        assert_eq!(bytes.len(), 104);
        assert_eq!(&bytes[..4], &[0x02, 0x02, 0x60, 0x00]);
    }

    #[test]
    fn encode_then_parse_round_trip() {
        let original = sample();
        let bytes = original.encode();
        let parsed = HearingAidSettings::parse(&bytes).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn payload_under_104_bytes_is_too_short() {
        let bytes = vec![0u8; 100];
        assert!(HearingAidSettings::parse(&bytes).is_err());
    }
}
