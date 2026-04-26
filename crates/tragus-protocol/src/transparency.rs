// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

//! Customize-Transparency payload codec (GATT handle `0x0018`).
//!
//! Wire layout from the LibrePods Android `Transparency.kt`:
//!
//! ```text
//! offset  field                                bytes  type
//! ------  -----                                -----  -----
//! 0       enabled (>0.5 == on)                 4      f32
//! 4–35    left  EQ (8 bands)                   32     [f32; 8]
//! 36      left  amplification                  4      f32
//! 40      left  tone                           4      f32
//! 44      left  conversation boost             4      f32
//! 48      left  ambient noise reduction        4      f32
//! 52–83   right EQ (8 bands)                   32     [f32; 8]
//! 84      right amplification                  4      f32
//! 88      right tone                           4      f32
//! 92      right conversation boost             4      f32
//! 96      right ambient noise reduction        4      f32
//! [100]   own-voice amplification (optional)   4      f32
//! ```
//!
//! Per-ear sub-layout (48 bytes) is shared with Hearing-Aid via
//! [`crate::channel`].

pub use crate::channel::{ChannelSettings as Channel, EqBands};
use crate::channel::{ChannelSettings, encode_channel, parse_channel, read_f32, write_f32};
use crate::error::ProtocolError;

/// GATT characteristic handle for Customize-Transparency.
pub const HANDLE: u16 = 0x0018;

const MIN_PAYLOAD_LEN: usize = 100;
const FULL_PAYLOAD_LEN: usize = 104;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransparencySettings {
    pub enabled: bool,
    pub left: ChannelSettings,
    pub right: ChannelSettings,
    /// Some firmware versions append a final f32 for own-voice
    /// amplification; older versions stop after 100 bytes.
    pub own_voice_amplification: Option<f32>,
}

impl TransparencySettings {
    pub fn parse(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < MIN_PAYLOAD_LEN {
            return Err(ProtocolError::TooShort {
                expected: MIN_PAYLOAD_LEN,
                got: bytes.len(),
            });
        }
        Ok(Self {
            enabled: read_f32(bytes, 0)? > 0.5,
            left: parse_channel(&bytes[4..52])?,
            right: parse_channel(&bytes[52..100])?,
            own_voice_amplification: if bytes.len() >= FULL_PAYLOAD_LEN {
                Some(read_f32(bytes, 100)?)
            } else {
                None
            },
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let len = if self.own_voice_amplification.is_some() {
            FULL_PAYLOAD_LEN
        } else {
            MIN_PAYLOAD_LEN
        };
        let mut buf = Vec::with_capacity(len);
        write_f32(&mut buf, if self.enabled { 1.0 } else { 0.0 });
        encode_channel(&mut buf, &self.left);
        encode_channel(&mut buf, &self.right);
        if let Some(v) = self.own_voice_amplification {
            write_f32(&mut buf, v);
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn att_handle_matches_spec() {
        assert_eq!(HANDLE, 0x0018);
    }

    fn sample() -> TransparencySettings {
        TransparencySettings {
            enabled: true,
            left: ChannelSettings {
                eq: EqBands {
                    bands: [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
                },
                amplification: 0.5,
                tone: 0.6,
                conversation_boost: 1.0,
                ambient_noise_reduction: 0.4,
            },
            right: ChannelSettings {
                eq: EqBands {
                    bands: [-0.1, -0.2, -0.3, -0.4, -0.5, -0.6, -0.7, -0.8],
                },
                amplification: 0.7,
                tone: 0.3,
                conversation_boost: 0.0,
                ambient_noise_reduction: 0.9,
            },
            own_voice_amplification: None,
        }
    }

    #[test]
    fn encode_then_parse_round_trip_without_own_voice() {
        let original = sample();
        let bytes = original.encode();
        assert_eq!(bytes.len(), 100);
        let parsed = TransparencySettings::parse(&bytes).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn encode_then_parse_round_trip_with_own_voice() {
        let mut s = sample();
        s.own_voice_amplification = Some(0.75);
        let bytes = s.encode();
        assert_eq!(bytes.len(), 104);
        let parsed = TransparencySettings::parse(&bytes).unwrap();
        assert_eq!(parsed, s);
    }

    #[test]
    fn enabled_flag_thresholds_at_half() {
        let mut bytes = sample().encode();
        bytes[..4].copy_from_slice(&0.4_f32.to_le_bytes());
        assert!(!TransparencySettings::parse(&bytes).unwrap().enabled);

        bytes[..4].copy_from_slice(&0.6_f32.to_le_bytes());
        assert!(TransparencySettings::parse(&bytes).unwrap().enabled);
    }

    #[test]
    fn payload_under_100_bytes_is_too_short() {
        let bytes = vec![0u8; 50];
        assert!(TransparencySettings::parse(&bytes).is_err());
    }
}
