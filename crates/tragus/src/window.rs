// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Main window — vertical slice for M3.
//!
//! Plain `gtk::Label`s bound to [`AirPodsState`] properties. The point
//! of this iteration is to prove the data flow end-to-end:
//!
//!   bluer / fake → channel → bridge → AirPodsState → property binding
//!   → label text
//!
//! A polished layout (battery cards, ANC toggle group, header status)
//! lands in M3.F once the plumbing is confirmed working against a
//! `--fake` run.

use crate::state::AirPodsState;
use adw::prelude::*;
use gtk::glib;

pub fn build_ui(app: &adw::Application, state: &AirPodsState) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Tragus")
        .default_width(420)
        .default_height(560)
        .build();

    let header = adw::HeaderBar::new();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();

    content.append(&connection_label(state));
    content.append(&battery_label(state, "battery-left", "Left"));
    content.append(&battery_label(state, "battery-right", "Right"));
    content.append(&battery_label(state, "battery-case", "Case"));
    content.append(&listening_mode_label(state));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&content));

    window.set_content(Some(&toolbar));
    window.present();
}

fn connection_label(state: &AirPodsState) -> gtk::Label {
    let label = gtk::Label::builder().halign(gtk::Align::Start).build();
    state
        .bind_property("connected", &label, "label")
        .transform_to(|_, connected: bool| {
            Some(
                if connected {
                    "Connected"
                } else {
                    "Disconnected"
                }
                .to_value(),
            )
        })
        .sync_create()
        .build();
    label
}

fn battery_label(state: &AirPodsState, prop: &str, prefix: &str) -> gtk::Label {
    let label = gtk::Label::builder().halign(gtk::Align::Start).build();
    let prefix = prefix.to_string();
    state
        .bind_property(prop, &label, "label")
        .transform_to(move |_, level: i32| {
            Some(
                if level < 0 {
                    format!("{prefix}: —")
                } else {
                    format!("{prefix}: {level}%")
                }
                .to_value(),
            )
        })
        .sync_create()
        .build();
    label
}

fn listening_mode_label(state: &AirPodsState) -> gtk::Label {
    let label = gtk::Label::builder().halign(gtk::Align::Start).build();
    state
        .bind_property("listening-mode", &label, "label")
        .transform_to(|_, mode: i32| {
            let text = match mode {
                0x01 => "ANC: Off",
                0x02 => "ANC: Noise Cancellation",
                0x03 => "ANC: Transparency",
                0x04 => "ANC: Adaptive",
                _ => "ANC: —",
            };
            Some(text.to_value())
        })
        .sync_create()
        .build();
    label
}

// `glib::Value` is needed for transform_to closures' return type, which
// the trait's signature requires us to reference even though we don't
// touch its constructors directly.
#[allow(unused_imports)]
use glib::Value;
