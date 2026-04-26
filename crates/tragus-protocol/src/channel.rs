// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

//! Per-ear channel settings shared by Customize-Transparency
//! (handle `0x0018`) and Hearing-Aid (handle `0x002A`). Both use an
//! identical 48-byte sub-layout: 8-band EQ + amplification + tone +
//! conversation boost + ambient noise reduction, all IEEE-754 LE
//! `f32`.

use crate::error::ProtocolError;

pub const EQ_BANDS: usize = 8;
/// Length of one ear's serialised channel data.
pub const CHANNEL_LEN: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqBands {
    pub bands: [f32; EQ_BANDS],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelSettings {
    pub eq: EqBands,
    pub amplification: f32,
    pub tone: f32,
    pub conversation_boost: f32,
    pub ambient_noise_reduction: f32,
}

impl ChannelSettings {
    /// Flat / neutral settings — every band, amp, tone, etc. at 0.0.
    pub fn flat() -> Self {
        Self {
            eq: EqBands {
                bands: [0.0; EQ_BANDS],
            },
            amplification: 0.0,
            tone: 0.0,
            conversation_boost: 0.0,
            ambient_noise_reduction: 0.0,
        }
    }
}

/// Read an IEEE-754 LE `f32` at `offset`. Errors with `TooShort` if
/// the slice is shorter than `offset + 4`.
pub(crate) fn read_f32(bytes: &[u8], offset: usize) -> Result<f32, ProtocolError> {
    if bytes.len() < offset + 4 {
        return Err(ProtocolError::TooShort {
            expected: offset + 4,
            got: bytes.len(),
        });
    }
    Ok(f32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]))
}

pub(crate) fn write_f32(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Decode 48 bytes into a [`ChannelSettings`].
pub(crate) fn parse_channel(bytes: &[u8]) -> Result<ChannelSettings, ProtocolError> {
    if bytes.len() < CHANNEL_LEN {
        return Err(ProtocolError::TooShort {
            expected: CHANNEL_LEN,
            got: bytes.len(),
        });
    }
    let mut bands = [0.0f32; EQ_BANDS];
    for (i, slot) in bands.iter_mut().enumerate() {
        *slot = read_f32(bytes, i * 4)?;
    }
    Ok(ChannelSettings {
        eq: EqBands { bands },
        amplification: read_f32(bytes, 32)?,
        tone: read_f32(bytes, 36)?,
        conversation_boost: read_f32(bytes, 40)?,
        ambient_noise_reduction: read_f32(bytes, 44)?,
    })
}

pub(crate) fn encode_channel(buf: &mut Vec<u8>, ch: &ChannelSettings) {
    for v in ch.eq.bands {
        write_f32(buf, v);
    }
    write_f32(buf, ch.amplification);
    write_f32(buf, ch.tone);
    write_f32(buf, ch.conversation_boost);
    write_f32(buf, ch.ambient_noise_reduction);
}
