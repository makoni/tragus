// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Write-half of the daemon: drains commands from a channel and writes
//! the encoded bytes to an `AsyncWrite` (the L2CAP socket in production,
//! a `tokio_test::io::Builder` mock in tests).
//!
//! `DaemonCommand` is intentionally narrow today — it grows as the UI
//! needs it. `SendControlCommand` is the escape hatch for raw control
//! commands the typed variants don't yet cover.

use crate::TransportError;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tragus_protocol::control_command::{self, ControlCommand, ListeningMode};
use tragus_protocol::frame::Frame;
use tragus_protocol::rename;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonCommand {
    SetListeningMode(ListeningMode),
    SendControlCommand(ControlCommand),
    Rename(String),
}

impl DaemonCommand {
    fn encode(&self) -> Vec<u8> {
        match self {
            Self::SetListeningMode(mode) => {
                let cmd = ControlCommand::set_listening_mode(*mode);
                Frame::encode(control_command::OPCODE, &cmd.encode_payload())
            }
            Self::SendControlCommand(cmd) => {
                Frame::encode(control_command::OPCODE, &cmd.encode_payload())
            }
            Self::Rename(name) => Frame::encode(rename::OPCODE, &rename::encode_rename(name)),
        }
    }
}

pub async fn run_command_loop<W: AsyncWrite + Unpin>(
    writer: &mut W,
    commands: &async_channel::Receiver<DaemonCommand>,
) -> Result<(), TransportError> {
    while let Ok(cmd) = commands.recv().await {
        let bytes = cmd.encode();
        writer.write_all(&bytes).await?;
        writer.flush().await?;
    }
    // Sender dropped — UI shutting down. Clean exit.
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::command_loop::{DaemonCommand, run_command_loop};
    use tokio_test::io::Builder;
    use tragus_protocol::control_command::{
        ControlCommand, ControlIdentifier, EnabledDisabled, ListeningMode,
    };

    /// SetListeningMode(NoiseCancellation) → exact frame from spec:
    /// `04 00 04 00 09 00 0D 02 00 00 00`
    #[tokio::test]
    async fn set_listening_mode_writes_expected_frame() {
        let mut mock = Builder::new()
            .write(&[
                0x04, 0x00, 0x04, 0x00, 0x09, 0x00, 0x0D, 0x02, 0x00, 0x00, 0x00,
            ])
            .build();
        let (tx, rx) = async_channel::bounded(1);

        tx.send(DaemonCommand::SetListeningMode(
            ListeningMode::NoiseCancellation,
        ))
        .await
        .unwrap();
        drop(tx); // close channel — loop should exit Ok(())

        run_command_loop(&mut mock, &rx).await.unwrap();
    }

    #[tokio::test]
    async fn raw_control_command_passthrough() {
        // Disable ear detection: identifier 0x0A, value 0x02 (disabled).
        let mut mock = Builder::new()
            .write(&[
                0x04, 0x00, 0x04, 0x00, 0x09, 0x00, 0x0A, 0x02, 0x00, 0x00, 0x00,
            ])
            .build();
        let (tx, rx) = async_channel::bounded(1);

        tx.send(DaemonCommand::SendControlCommand(
            ControlCommand::set_toggle(
                ControlIdentifier::EarDetectionEnabled,
                EnabledDisabled::Disabled,
            ),
        ))
        .await
        .unwrap();
        drop(tx);

        run_command_loop(&mut mock, &rx).await.unwrap();
    }

    #[tokio::test]
    async fn rename_writes_expected_frame() {
        // 04 00 04 00 1A 00 01 04 00 'P' 'o' 'd' 's'
        let mut mock = Builder::new()
            .write(&[
                0x04, 0x00, 0x04, 0x00, 0x1A, 0x00, 0x01, 0x04, 0x00, b'P', b'o', b'd', b's',
            ])
            .build();
        let (tx, rx) = async_channel::bounded(1);

        tx.send(DaemonCommand::Rename("Pods".into())).await.unwrap();
        drop(tx);

        run_command_loop(&mut mock, &rx).await.unwrap();
    }

    #[tokio::test]
    async fn drains_multiple_commands_in_order() {
        let mut mock = Builder::new()
            .write(&[
                0x04, 0x00, 0x04, 0x00, 0x09, 0x00, 0x0D, 0x01, 0x00, 0x00, 0x00,
            ])
            .write(&[
                0x04, 0x00, 0x04, 0x00, 0x09, 0x00, 0x0D, 0x03, 0x00, 0x00, 0x00,
            ])
            .build();
        let (tx, rx) = async_channel::bounded(2);

        tx.send(DaemonCommand::SetListeningMode(ListeningMode::Off))
            .await
            .unwrap();
        tx.send(DaemonCommand::SetListeningMode(ListeningMode::Transparency))
            .await
            .unwrap();
        drop(tx);

        run_command_loop(&mut mock, &rx).await.unwrap();
    }
}
