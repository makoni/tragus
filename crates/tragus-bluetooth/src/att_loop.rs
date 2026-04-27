// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! ATT-side actor loop.
//!
//! Runs over the dedicated PSM-0x1F socket. Drives one-shot reads and
//! writes triggered by `AttCommand`s from the UI, and surfaces
//! incoming Notifications as `AttEvent`s.
//!
//! ## Request/response correlation
//!
//! ATT's `ReadResponse` doesn't carry the handle that was requested.
//! For now we serialise commands strictly (one outstanding request at
//! a time) and remember the pending characteristic in a single slot.
//! If we ever multiplex multiple in-flight reads we'll need a small
//! request queue.
//!
//! ## Why no unit test
//!
//! The loop is a `tokio::select!` over two arms; `tokio_test::io`
//! mocks deliver bytes deterministically but the select arm chosen
//! per await is racy in test, mirrors the same situation as
//! `daemon::run`. The pieces it composes (`read_att_pdu`,
//! `write_att_pdu`, `AttPdu::parse`/`encode`,
//! `TransparencySettings::parse`/`encode`) are each TDD-covered.

use crate::TransportError;
use crate::att_session::{read_att_pdu, write_att_pdu};
use std::cell::Cell;
use tokio::io::{AsyncRead, AsyncWrite};
use tragus_protocol::att::AttPdu;
use tragus_protocol::hearing_aid_settings::{self, HearingAidSettings};
use tragus_protocol::transparency::{self, TransparencySettings};

#[derive(Debug, Clone)]
pub enum AttCommand {
    /// Read the current transparency settings.
    ReadTransparency,
    /// Write new transparency settings.
    WriteTransparency(TransparencySettings),
    /// Read the current hearing-aid settings.
    ReadHearingAid,
    /// Write new hearing-aid settings.
    WriteHearingAid(HearingAidSettings),
}

#[derive(Debug, Clone)]
pub enum AttEvent {
    /// Result of a `ReadTransparency` round trip.
    TransparencyRead(TransparencySettings),
    /// Notification fired by the AirPods when transparency settings
    /// change on-device (e.g. via the iPhone or another connected host).
    TransparencyChanged(TransparencySettings),
    /// Result of a `ReadHearingAid` round trip.
    HearingAidRead(HearingAidSettings),
    /// Notification fired when hearing-aid settings change.
    HearingAidChanged(HearingAidSettings),
}

#[derive(Debug, Clone, Copy)]
enum PendingRead {
    None,
    Transparency,
    HearingAid,
}

pub async fn run_att_loop<S>(
    socket: S,
    commands: async_channel::Receiver<AttCommand>,
    events: async_channel::Sender<AttEvent>,
) -> Result<(), TransportError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let (mut reader, mut writer) = tokio::io::split(socket);
    let pending: Cell<PendingRead> = Cell::new(PendingRead::None);

    loop {
        tokio::select! {
            cmd = commands.recv() => match cmd {
                Ok(AttCommand::ReadTransparency) => {
                    tracing::debug!("↓ AttCommand::ReadTransparency");
                    pending.set(PendingRead::Transparency);
                    write_att_pdu(
                        &mut writer,
                        &AttPdu::ReadRequest { handle: transparency::HANDLE },
                    ).await?;
                }
                Ok(AttCommand::WriteTransparency(settings)) => {
                    tracing::debug!(?settings, "↓ AttCommand::WriteTransparency");
                    write_att_pdu(
                        &mut writer,
                        &AttPdu::WriteRequest {
                            handle: transparency::HANDLE,
                            value: settings.encode(),
                        },
                    ).await?;
                }
                Ok(AttCommand::ReadHearingAid) => {
                    tracing::debug!("↓ AttCommand::ReadHearingAid");
                    pending.set(PendingRead::HearingAid);
                    write_att_pdu(
                        &mut writer,
                        &AttPdu::ReadRequest { handle: hearing_aid_settings::HANDLE },
                    ).await?;
                }
                Ok(AttCommand::WriteHearingAid(settings)) => {
                    tracing::debug!(?settings, "↓ AttCommand::WriteHearingAid");
                    write_att_pdu(
                        &mut writer,
                        &AttPdu::WriteRequest {
                            handle: hearing_aid_settings::HANDLE,
                            value: settings.encode(),
                        },
                    ).await?;
                }
                Err(_) => {
                    tracing::debug!("ATT command channel closed; exiting ATT loop");
                    return Ok(());
                }
            },
            pdu = read_att_pdu(&mut reader) => match pdu? {
                AttPdu::ReadResponse { value } => {
                    let event = match pending.replace(PendingRead::None) {
                        PendingRead::Transparency => TransparencySettings::parse(&value)
                            .ok()
                            .map(AttEvent::TransparencyRead),
                        PendingRead::HearingAid => HearingAidSettings::parse(&value)
                            .ok()
                            .map(AttEvent::HearingAidRead),
                        PendingRead::None => {
                            tracing::debug!(
                                "ATT ReadResponse with no pending request, ignoring"
                            );
                            None
                        }
                    };
                    if let Some(event) = event {
                        tracing::debug!(?event, "↑ AttEvent");
                        if events.send(event).await.is_err() {
                            return Ok(());
                        }
                    }
                }
                AttPdu::Notification { handle, value } if handle == transparency::HANDLE => {
                    if let Ok(s) = TransparencySettings::parse(&value)
                        && events.send(AttEvent::TransparencyChanged(s)).await.is_err()
                    {
                        return Ok(());
                    }
                }
                AttPdu::Notification { handle, value } if handle == hearing_aid_settings::HANDLE => {
                    if let Ok(s) = HearingAidSettings::parse(&value)
                        && events.send(AttEvent::HearingAidChanged(s)).await.is_err()
                    {
                        return Ok(());
                    }
                }
                AttPdu::Notification { handle, .. } => {
                    tracing::debug!("ATT notification for unhandled handle 0x{handle:04x}");
                }
                AttPdu::ErrorResponse { error, handle, request_opcode } => {
                    tracing::warn!(
                        "ATT error: opcode 0x{request_opcode:02x} on handle 0x{handle:04x}: {error:?}"
                    );
                }
                AttPdu::WriteResponse => {
                    // Successful write; nothing to surface.
                }
                other => {
                    tracing::debug!("unexpected inbound ATT PDU: {other:?}");
                }
            },
        }
    }
}
