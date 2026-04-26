// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

use adw::prelude::*;
use gtk::glib;

mod bridge;
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

        bridge::attach_event_stream(state.clone(), events_rx);

        if cli.fake {
            tracing::info!("starting in --fake mode (no Bluetooth)");
            state.set_connected(true);
            fake::spawn_fake_source(events_tx);
        } else {
            // Real bluer integration lands in M3.G. Until then a non-fake
            // launch shows the disconnected StatusPage.
            tracing::warn!("no daemon wired up yet; relaunch with --fake for a live UI demo");
            drop(events_tx);
        }

        window::build_ui(app, &state);
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
