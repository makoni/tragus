// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

//! Bluetooth transport for AAP.
//!
//! Wraps `bluer` and the L2CAP socket so the rest of the app sees a typed
//! async stream of protocol events instead of raw bytes.

pub mod event;
pub mod framing;
pub mod handshake;
pub mod read_loop;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("bluer error: {0}")]
    Bluer(#[from] bluer::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol error: {0}")]
    Protocol(#[from] tragus_protocol::ProtocolError),
    #[error("connection closed by peer")]
    ConnectionClosed,
}
