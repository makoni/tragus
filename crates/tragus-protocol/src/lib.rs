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

pub mod battery;
pub mod control_command;
pub mod ear_detection;
pub mod error;
pub mod feature_flags;
pub mod frame;
pub mod notifications;
pub mod rename;

pub use error::ProtocolError;
pub use frame::{Frame, OwnedFrame};

/// L2CAP PSM that AirPods listen on for AAP traffic.
pub const AAP_PSM: u16 = 0x1001;

/// First packet to send after the L2CAP socket is up. Without it, AirPods
/// stay silent. The handshake uses a different prefix from a regular AAP
/// frame, so it lives here as a top-level constant rather than under any
/// opcode module.
pub const HANDSHAKE: &[u8] = &[
    0x00, 0x00, 0x04, 0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
