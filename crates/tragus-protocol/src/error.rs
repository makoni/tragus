// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

use thiserror::Error;

/// Anything that can go wrong while parsing a byte slice as an AAP frame
/// or one of its typed payloads.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProtocolError {
    #[error("packet too short: expected at least {expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },

    #[error("invalid AAP frame prefix")]
    InvalidPrefix,

    #[error("unknown opcode: 0x{0:02x}")]
    UnknownOpcode(u8),

    #[error("unknown battery component: 0x{0:02x}")]
    UnknownBatteryComponent(u8),

    #[error("unknown battery status: 0x{0:02x}")]
    UnknownBatteryStatus(u8),

    #[error("unexpected byte at offset {offset}: expected 0x{expected:02x}, got 0x{got:02x}")]
    UnexpectedByte {
        offset: usize,
        expected: u8,
        got: u8,
    },
}
