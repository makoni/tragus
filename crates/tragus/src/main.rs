// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

use adw::prelude::*;
use gtk::glib;

#[allow(
    dead_code,
    reason = "wired up to the daemon bridge in a later M3 slice"
)]
mod media_state;
#[allow(
    dead_code,
    reason = "wired up to the daemon bridge in a later M3 slice"
)]
mod model;
#[allow(
    dead_code,
    reason = "wired up to the daemon bridge in a later M3 slice"
)]
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

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(window::build_ui);
    app.run()
}
