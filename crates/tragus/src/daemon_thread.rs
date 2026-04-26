// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Run the AAP daemon on a dedicated tokio runtime in a worker thread.
//!
//! GTK owns the main thread's event loop, so we can't `block_on` there
//! without freezing the UI. We park a single-threaded tokio runtime in
//! a `std::thread::spawn`, hand it the channels, and let it own
//! discovery → L2CAP connect → `daemon::run` for the lifetime of the
//! application.
//!
//! Reconnect on `ConnectionClosed` happens inside the worker too: if
//! the AirPods drop the link (taken out of range, low battery, etc.)
//! we sleep briefly and try again, so the UI just sees a brief
//! `connected = false` flicker.

use async_channel::{Receiver, Sender};
use std::time::Duration;
use tragus_bluetooth::command_loop::DaemonCommand;
use tragus_bluetooth::daemon;
use tragus_bluetooth::discovery::connect_first_paired_airpods;
use tragus_bluetooth::event::DaemonEvent;

/// Spawn the worker thread. Returns immediately. `connected` reports
/// transport-level connect / disconnect transitions to the UI.
pub fn spawn(
    events: Sender<DaemonEvent>,
    commands: Receiver<DaemonCommand>,
    connected: Sender<bool>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("tragus-bluetooth".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("could not build tokio runtime: {e}");
                    return;
                }
            };
            rt.block_on(run_loop(events, commands, connected));
        })
        .expect("spawn tragus-bluetooth thread")
}

async fn run_loop(
    events: Sender<DaemonEvent>,
    commands: Receiver<DaemonCommand>,
    connected: Sender<bool>,
) {
    loop {
        match try_one_session(&events, &commands, &connected).await {
            Ok(()) => {
                tracing::info!("daemon exited cleanly; UI is shutting down");
                return;
            }
            Err(reason) => {
                tracing::warn!("daemon session ended: {reason}; reconnecting in 3s");
                let _ = connected.send(false).await;
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
}

async fn try_one_session(
    events: &Sender<DaemonEvent>,
    commands: &Receiver<DaemonCommand>,
    connected: &Sender<bool>,
) -> Result<(), String> {
    let (address, socket) = connect_first_paired_airpods()
        .await
        .map_err(|e| format!("connect: {e}"))?;
    tracing::info!("connected to AirPods at {address}");

    let _ = connected.send(true).await;

    daemon::run(socket, commands.clone(), events.clone())
        .await
        .map_err(|e| format!("daemon: {e}"))
}
