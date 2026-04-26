// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

//! AAP protocol parsing and packet construction.
//!
//! This crate is pure Rust — it knows nothing about Bluetooth, BlueZ, or GTK.
//! It takes byte slices in and gives typed events out, and vice versa. Keeping
//! it transport-free makes it trivial to fuzz and unit-test.

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("packet too short: expected at least {expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown opcode: 0x{0:02x}")]
    UnknownOpcode(u8),
}

/// L2CAP PSM that AirPods listen on for AAP traffic.
pub const AAP_PSM: u16 = 0x1001;

/// First packet to send after the L2CAP socket is up. Without it, AirPods
/// stay silent.
pub const HANDSHAKE: &[u8] = &[
    0x00, 0x00, 0x04, 0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Subscribes to battery / ear-detection / ANC mode notifications.
pub const REQUEST_NOTIFICATIONS: &[u8] =
    &[0x04, 0x00, 0x04, 0x00, 0x0F, 0x00, 0xFF, 0xFF, 0xFE, 0xFF];
