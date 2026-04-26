// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

use adw::prelude::*;
use gtk::glib;

mod bridge;
mod daemon_thread;
mod fake;
mod media_state;
mod model;
mod mpris;
mod state;
mod window;

const APP_ID: &str = "me.spaceinbox.tragus";

fn main() -> glib::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tragus=debug,info".into()),
        )
        .init();

    let cli = parse_cli();

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| {
        let state = state::AirPodsState::new();
        let (events_tx, events_rx) = async_channel::bounded(64);
        let (commands_tx, commands_rx) = async_channel::bounded(8);
        let (att_events_tx, att_events_rx) = async_channel::bounded(16);
        let (att_commands_tx, att_commands_rx) = async_channel::bounded(8);

        bridge::attach_event_stream(state.clone(), events_rx);

        if cli.fake {
            tracing::info!("starting in --fake mode (no Bluetooth)");
            state.set_connected(true);
            fake::spawn_fake_source(events_tx);
            // Drain channels so the UI never blocks on a full channel.
            // In a real run daemon_thread owns both ends of these.
            drain_in_fake_mode(commands_rx, att_commands_rx, att_events_tx);
        } else {
            tracing::info!("starting bluer daemon thread");
            let (connected_tx, connected_rx) = async_channel::bounded(4);
            daemon_thread::spawn(
                events_tx,
                commands_rx,
                att_events_tx,
                att_commands_rx,
                connected_tx,
            );

            let state_for_connected = state.clone();
            glib::spawn_future_local(async move {
                while let Ok(c) = connected_rx.recv().await {
                    state_for_connected.set_connected(c);
                }
            });
        }

        // ATT events go to the bridge / future Customize page; for now
        // we just log them so the channel doesn't fill.
        glib::spawn_future_local(async move {
            while let Ok(e) = att_events_rx.recv().await {
                tracing::debug!(?e, "ATT event (no UI consumer yet)");
            }
        });

        window::build_ui(app, &state, commands_tx, att_commands_tx);
    });
    app.run()
}

fn drain_in_fake_mode(
    commands_rx: async_channel::Receiver<tragus_bluetooth::command_loop::DaemonCommand>,
    att_commands_rx: async_channel::Receiver<tragus_bluetooth::att_loop::AttCommand>,
    att_events_tx: async_channel::Sender<tragus_bluetooth::att_loop::AttEvent>,
) {
    glib::spawn_future_local(async move {
        while let Ok(cmd) = commands_rx.recv().await {
            tracing::debug!(?cmd, "swallowed in --fake mode");
        }
    });
    glib::spawn_future_local(async move {
        while let Ok(cmd) = att_commands_rx.recv().await {
            tracing::debug!(?cmd, "ATT command swallowed in --fake mode");
        }
    });
    // Keep the sender alive until the app exits.
    drop(att_events_tx);
}

#[derive(Debug, Default, Clone, Copy)]
struct Cli {
    fake: bool,
}

fn parse_cli() -> Cli {
    let mut cli = Cli::default();
    for arg in std::env::args().skip(1) {
        if arg == "--fake" {
            cli.fake = true;
        }
    }
    if std::env::var("TRAGUS_FAKE").is_ok_and(|v| !v.is_empty() && v != "0") {
        cli.fake = true;
    }
    cli
}
