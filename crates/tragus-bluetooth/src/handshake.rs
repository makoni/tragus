// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! AAP initial-sequence sender.
//!
//! After the L2CAP socket to PSM 0x1001 is open, the AirPods stay silent
//! until they see this exact three-packet sequence:
//!
//! 1. **HANDSHAKE** — Apple's magic preamble; the AirPods reject every
//!    other opcode until they see it.
//! 2. **REQUEST_NOTIFICATIONS** — subscribes us to battery / ear
//!    detection / ANC mode pushes.
//! 3. **SET_FEATURE_FLAGS** — opaque feature mask. Required only on
//!    AirPods Pro 2 to enable conv. awareness during audio playback,
//!    but cheap and harmless on other models, so we send it
//!    unconditionally until model detection is wired up.
//!
//! The function is generic over `AsyncWrite` so it can be unit-tested
//! against `tokio_test::io::Builder` mocks without ever touching BlueZ
//! or hardware.

use tokio::io::{AsyncWrite, AsyncWriteExt};
use tragus_protocol::{
    HANDSHAKE, feature_flags::SetFeatureFlags, frame::Frame, notifications::NotificationFlags,
};

/// Send the three-packet AAP initialisation sequence over an open L2CAP
/// socket (or any `AsyncWrite`).
pub async fn send_initial_sequence<W: AsyncWrite + Unpin>(writer: &mut W) -> std::io::Result<()> {
    tracing::info!("sending AAP init sequence");

    tracing::trace!(bytes = ?HANDSHAKE, "→ HANDSHAKE");
    writer.write_all(HANDSHAKE).await?;

    let request_notifications = Frame::encode(
        tragus_protocol::notifications::OPCODE,
        &NotificationFlags::ALL.encode_payload(),
    );
    tracing::trace!(bytes = ?request_notifications, "→ REQUEST_NOTIFICATIONS");
    writer.write_all(&request_notifications).await?;

    let set_feature_flags = Frame::encode(
        tragus_protocol::feature_flags::OPCODE,
        &SetFeatureFlags::PRO2_DEFAULT.encode_payload(),
    );
    tracing::trace!(bytes = ?set_feature_flags, "→ SET_FEATURE_FLAGS (Pro 2 default)");
    writer.write_all(&set_feature_flags).await?;

    writer.flush().await?;
    tracing::info!("AAP init sequence flushed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::handshake::send_initial_sequence;
    use tokio_test::io::Builder;

    /// Three packets in fixed order — handshake, request-notifications,
    /// set-feature-flags — each captured byte-for-byte from
    /// `AAP Definitions.md`. The mock writer asserts that exactly these
    /// bytes appear in exactly this sequence.
    #[tokio::test]
    async fn writes_three_init_packets_in_order() {
        const HANDSHAKE: &[u8] = &[
            0x00, 0x00, 0x04, 0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        const REQUEST_NOTIFICATIONS_ALL: &[u8] =
            &[0x04, 0x00, 0x04, 0x00, 0x0F, 0x00, 0xFF, 0xFF, 0xFF, 0xFF];
        const SET_FEATURE_FLAGS_PRO2: &[u8] = &[
            0x04, 0x00, 0x04, 0x00, 0x4D, 0x00, 0xD7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let mut mock = Builder::new()
            .write(HANDSHAKE)
            .write(REQUEST_NOTIFICATIONS_ALL)
            .write(SET_FEATURE_FLAGS_PRO2)
            .build();

        send_initial_sequence(&mut mock).await.unwrap();
    }
}
