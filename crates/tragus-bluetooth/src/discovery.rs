// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Find paired AirPods on a BlueZ adapter, then open an L2CAP socket
//! to AAP PSM `0x1001`.
//!
//! ## Layering
//!
//! - [`looks_like_airpods`] — pure helper, name-based heuristic, fully
//!   unit-tested.
//! - [`find_paired_airpods`] — touches `bluer::Adapter`. No unit tests:
//!   `bluer` reaches D-Bus on construction; mocking it would require a
//!   trait abstraction that isn't worth its weight today. Manual
//!   integration test: `cargo run -p tragus` with a paired pair nearby.
//! - [`open_aap_socket`] — wraps `bluer::l2cap::Stream::connect`. Same
//!   reasoning.
//!
//! ## Permissions
//!
//! On stock BlueZ ≥ 5.56, opening L2CAP by PSM does **not** require
//! `CAP_NET_RAW`. On older systems users may need
//! `setcap cap_net_raw+eip ./tragus` once.

use crate::TransportError;
use bluer::l2cap::{SocketAddr, Stream};
use bluer::{Adapter, Address, AddressType, Device, Session};
use tragus_protocol::{AAP_PSM, ATT_PSM};

/// True if the device name looks like AirPods. Heuristic by design —
/// AirPods are renameable, but the default name shape (`"AirPods"`,
/// `"AirPods Pro"`, `"<owner>'s AirPods"`, etc.) reliably contains the
/// substring. False positives just mean we'll try a handshake against
/// something that won't speak AAP, and the L2CAP connect will fail
/// quickly.
pub fn looks_like_airpods(name: Option<&str>) -> bool {
    name.is_some_and(|n| n.contains("AirPods"))
}

/// Iterate over paired devices on this adapter and return those whose
/// name passes [`looks_like_airpods`].
pub async fn find_paired_airpods(adapter: &Adapter) -> Result<Vec<Device>, TransportError> {
    let mut found = Vec::new();
    for addr in adapter.device_addresses().await? {
        let device = adapter.device(addr)?;
        if !device.is_paired().await.unwrap_or(false) {
            continue;
        }
        if looks_like_airpods(device.name().await?.as_deref()) {
            found.push(device);
        }
    }
    Ok(found)
}

/// Open an L2CAP stream to the AAP PSM on the given AirPods.
pub async fn open_aap_socket(addr: Address) -> Result<Stream, TransportError> {
    let target = SocketAddr::new(addr, AddressType::BrEdr, AAP_PSM);
    Ok(Stream::connect(target).await?)
}

/// Open the ATT-side L2CAP stream for the GATT characteristics
/// (transparency, hearing aid, loud-sound reduction).
pub async fn open_att_socket(addr: Address) -> Result<Stream, TransportError> {
    let target = SocketAddr::new(addr, AddressType::BrEdr, ATT_PSM);
    Ok(Stream::connect(target).await?)
}

/// One-shot helper that wires the whole sequence together: open a
/// `bluer::Session`, pick the default adapter, find the first paired
/// AirPods, and open an AAP socket to it. Used by the application's
/// background daemon thread.
pub async fn connect_first_paired_airpods() -> Result<(Address, Stream), TransportError> {
    let session = Session::new().await?;
    let adapter = session.default_adapter().await?;
    let pairs = find_paired_airpods(&adapter).await?;
    let device = pairs.first().ok_or(TransportError::NoAirPodsFound)?;
    let address = device.address();
    let socket = open_aap_socket(address).await?;
    Ok((address, socket))
}

#[cfg(test)]
mod tests {
    use super::looks_like_airpods;

    #[test]
    fn matches_default_airpods_names() {
        assert!(looks_like_airpods(Some("AirPods")));
        assert!(looks_like_airpods(Some("AirPods Pro")));
        assert!(looks_like_airpods(Some("AirPods Pro 2 (USB-C)")));
        assert!(looks_like_airpods(Some("AirPods Max")));
        assert!(looks_like_airpods(Some("Sergey's AirPods")));
    }

    #[test]
    fn rejects_other_audio_devices() {
        assert!(!looks_like_airpods(Some("Beats Studio")));
        assert!(!looks_like_airpods(Some("Sony WH-1000XM5")));
        assert!(!looks_like_airpods(Some("BOSE QC45")));
    }

    #[test]
    fn rejects_missing_or_empty_name() {
        assert!(!looks_like_airpods(None));
        assert!(!looks_like_airpods(Some("")));
    }
}
