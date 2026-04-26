// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Run the AAP daemon plus the ATT loop on a dedicated tokio runtime
//! in a worker thread.
//!
//! GTK owns the main thread's event loop, so we can't `block_on`
//! there. We park a single-threaded tokio runtime in a
//! `std::thread::spawn`, hand it the four channels (AAP commands /
//! events, ATT commands / events) plus a connected flag for the UI,
//! and let the runtime own the lifecycle: discover → open both
//! sockets → run AAP daemon and ATT loop concurrently.
//!
//! If either loop returns we tear the session down, sleep briefly, and
//! retry. The UI sees a `connected = false` flicker on reconnects.

use async_channel::{Receiver, Sender};
use std::time::Duration;
use tragus_bluetooth::att_loop::{AttCommand, AttEvent, run_att_loop};
use tragus_bluetooth::command_loop::DaemonCommand;
use tragus_bluetooth::daemon;
use tragus_bluetooth::discovery::{connect_first_paired_airpods, open_att_socket};
use tragus_bluetooth::event::DaemonEvent;

pub fn spawn(
    events: Sender<DaemonEvent>,
    commands: Receiver<DaemonCommand>,
    att_events: Sender<AttEvent>,
    att_commands: Receiver<AttCommand>,
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
            rt.block_on(run_loop(
                events,
                commands,
                att_events,
                att_commands,
                connected,
            ));
        })
        .expect("spawn tragus-bluetooth thread")
}

async fn run_loop(
    events: Sender<DaemonEvent>,
    commands: Receiver<DaemonCommand>,
    att_events: Sender<AttEvent>,
    att_commands: Receiver<AttCommand>,
    connected: Sender<bool>,
) {
    loop {
        match try_one_session(&events, &commands, &att_events, &att_commands, &connected).await {
            Ok(()) => {
                tracing::info!("session exited cleanly; UI is shutting down");
                return;
            }
            Err(reason) => {
                tracing::warn!("session ended: {reason}; reconnecting in 3s");
                let _ = connected.send(false).await;
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
}

async fn try_one_session(
    events: &Sender<DaemonEvent>,
    commands: &Receiver<DaemonCommand>,
    att_events: &Sender<AttEvent>,
    att_commands: &Receiver<AttCommand>,
    connected: &Sender<bool>,
) -> Result<(), String> {
    let (address, aap_socket) = connect_first_paired_airpods()
        .await
        .map_err(|e| format!("AAP connect: {e}"))?;
    tracing::info!("AAP socket open to {address}");

    let att_socket = open_att_socket(address)
        .await
        .map_err(|e| format!("ATT connect: {e}"))?;
    tracing::info!("ATT socket open to {address}");

    let _ = connected.send(true).await;

    let aap_fut = daemon::run(aap_socket, commands.clone(), events.clone());
    let att_fut = run_att_loop(att_socket, att_commands.clone(), att_events.clone());

    tokio::select! {
        r = aap_fut => r.map_err(|e| format!("daemon: {e}")),
        r = att_fut => r.map_err(|e| format!("att: {e}")),
    }
}
