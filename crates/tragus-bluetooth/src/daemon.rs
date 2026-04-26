// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Daemon glue.
//!
//! Owns an open AAP-speaking socket end-to-end: sends the init sequence,
//! then runs the read- and command-loops concurrently. The two loops
//! share a `tokio::select!`, so the daemon exits as soon as either
//! finishes — typically because the socket closed (returns
//! `Err(ConnectionClosed)`) or the UI dropped both channel halves
//! (returns `Ok(())`).
//!
//! No unit tests here on purpose: the three subcomponents
//! (`handshake::send_initial_sequence`, `read_loop::run_read_loop`,
//! `command_loop::run_command_loop`) are each TDD-covered against
//! `tokio_test::io::Builder` mocks. `tokio::io::split` plus a `select!`
//! on top is straightforward orchestration; testing it as one unit
//! would race on which side of the select polls first.

use crate::TransportError;
use crate::command_loop::{DaemonCommand, run_command_loop};
use crate::event::DaemonEvent;
use crate::handshake::send_initial_sequence;
use crate::read_loop::run_read_loop;
use tokio::io::{AsyncRead, AsyncWrite};

pub async fn run<S>(
    socket: S,
    commands: async_channel::Receiver<DaemonCommand>,
    events: async_channel::Sender<DaemonEvent>,
) -> Result<(), TransportError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let (mut reader, mut writer) = tokio::io::split(socket);

    send_initial_sequence(&mut writer).await?;

    tokio::select! {
        r = run_read_loop(&mut reader, &events) => r,
        c = run_command_loop(&mut writer, &commands) => c,
    }
}
