// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

use adw::prelude::*;

pub fn build_ui(app: &adw::Application) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Tragus")
        .default_width(420)
        .default_height(560)
        .build();

    let header = adw::HeaderBar::new();

    let status = adw::StatusPage::builder()
        .icon_name("audio-headphones-symbolic")
        .title("No AirPods connected")
        .description("Make sure your AirPods are paired in GNOME Bluetooth settings.")
        .build();

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&status));

    window.set_content(Some(&toolbar));
    window.present();
}
