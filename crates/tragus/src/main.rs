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
#[allow(dead_code, reason = "wired into the bridge once MPRIS lands in M3.F")]
mod media_state;
mod model;
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

        bridge::attach_event_stream(state.clone(), events_rx);

        if cli.fake {
            tracing::info!("starting in --fake mode (no Bluetooth)");
            state.set_connected(true);
            fake::spawn_fake_source(events_tx);
            // Drain commands so the UI never blocks on a full channel.
            // In a real run M3.G hands commands_rx to the daemon.
            glib::spawn_future_local(async move {
                while let Ok(cmd) = commands_rx.recv().await {
                    tracing::debug!(?cmd, "swallowed in --fake mode");
                }
            });
        } else {
            tracing::info!("starting bluer daemon thread");
            let (connected_tx, connected_rx) = async_channel::bounded(4);
            daemon_thread::spawn(events_tx, commands_rx, connected_tx);

            let state_for_connected = state.clone();
            glib::spawn_future_local(async move {
                while let Ok(c) = connected_rx.recv().await {
                    state_for_connected.set_connected(c);
                }
            });
        }

        window::build_ui(app, &state, commands_tx);
    });
    app.run()
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
