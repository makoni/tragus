// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Main window — Adwaita-styled vertical slice for M3.
//!
//! Two `PreferencesGroup`s:
//!   - **Battery** — three `ActionRow`s for Left / Right / Case, each
//!     with a percentage label + charging icon bound to the matching
//!     `AirPodsState` properties.
//!   - **Noise Control** — four `ToggleButton`s (Off / Noise Cancellation
//!     / Transparency / Adaptive) inside a `linked`-styled box (segmented
//!     control look). Clicks send `DaemonCommand::SetListeningMode` on
//!     the command channel.
//!
//! When `state.connected == false`, both cards are hidden and a
//! StatusPage takes their place. Toggle is a single `gtk::Stack`
//! switched by `connected` via property binding.

use crate::state::AirPodsState;
use adw::prelude::*;
use gtk::glib;
use std::cell::Cell;
use std::rc::Rc;
use tragus_bluetooth::command_loop::DaemonCommand;
use tragus_protocol::control_command::{ClickHoldAction, ControlCommand, ListeningMode};

/// Channel into the daemon. Cloned per click handler.
pub type CommandSender = async_channel::Sender<DaemonCommand>;

pub fn build_ui(app: &adw::Application, state: &AirPodsState, commands: CommandSender) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Tragus")
        .default_width(420)
        .default_height(560)
        .build();

    let header = adw::HeaderBar::new();

    let rename_button = gtk::Button::builder()
        .icon_name("document-edit-symbolic")
        .tooltip_text("Rename AirPods")
        .build();
    {
        let commands = commands.clone();
        let window_weak = window.downgrade();
        rename_button.connect_clicked(move |_| {
            if let Some(window) = window_weak.upgrade() {
                show_rename_dialog(&window, commands.clone());
            }
        });
    }
    header.pack_end(&rename_button);

    state
        .bind_property("connected", &rename_button, "sensitive")
        .sync_create()
        .build();

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .build();

    stack.add_named(&disconnected_view(), Some("disconnected"));
    stack.add_named(&connected_view(state, commands), Some("connected"));
    stack.set_visible_child_name("disconnected");

    state
        .bind_property("connected", &stack, "visible-child-name")
        .transform_to(|_, connected: bool| {
            Some(
                if connected {
                    "connected"
                } else {
                    "disconnected"
                }
                .to_value(),
            )
        })
        .sync_create()
        .build();

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&stack));

    window.set_content(Some(&toolbar));
    window.present();
}

fn disconnected_view() -> adw::StatusPage {
    adw::StatusPage::builder()
        .icon_name("audio-headphones-symbolic")
        .title("No AirPods connected")
        .description("Make sure your AirPods are paired in GNOME Bluetooth settings.")
        .build()
}

fn connected_view(state: &AirPodsState, commands: CommandSender) -> gtk::Widget {
    let clamp = adw::Clamp::builder().maximum_size(500).build();

    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(24)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(12)
        .margin_end(12)
        .build();

    column.append(&battery_group(state));
    column.append(&noise_control_group(state, commands.clone()));
    column.append(&long_press_group(commands));

    clamp.set_child(Some(&column));
    clamp.upcast()
}

fn long_press_group(commands: CommandSender) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Long Press")
        .description("Action when you press and hold the stem")
        .build();

    // Shared state: 0 = NoiseControl, 1 = Siri. Default to NoiseControl
    // until the AirPods echo back their current setting (notification
    // routing for ClickHoldMode lands in a later slice).
    let left_idx = Rc::new(Cell::new(0u32));
    let right_idx = Rc::new(Cell::new(0u32));

    group.add(&long_press_row(
        "Left",
        Rc::clone(&left_idx),
        Rc::clone(&right_idx),
        true,
        commands.clone(),
    ));
    group.add(&long_press_row(
        "Right",
        Rc::clone(&right_idx),
        Rc::clone(&left_idx),
        false,
        commands,
    ));
    group
}

fn long_press_row(
    title: &str,
    own_idx: Rc<Cell<u32>>,
    other_idx: Rc<Cell<u32>>,
    own_is_left: bool,
    commands: CommandSender,
) -> adw::ComboRow {
    let row = adw::ComboRow::builder().title(title).build();
    let model = gtk::StringList::new(&["Noise Control", "Siri"]);
    row.set_model(Some(&model));
    row.set_selected(own_idx.get());

    row.connect_selected_notify(move |row| {
        let selected = row.selected();
        own_idx.set(selected);
        let (left, right) = if own_is_left {
            (own_idx.get(), other_idx.get())
        } else {
            (other_idx.get(), own_idx.get())
        };
        let cmd =
            ControlCommand::set_click_hold_mode(index_to_action(right), index_to_action(left));
        let commands = commands.clone();
        glib::spawn_future_local(async move {
            if let Err(e) = commands.send(DaemonCommand::SendControlCommand(cmd)).await {
                tracing::warn!("dropping long-press command: {e}");
            }
        });
    });
    row
}

fn index_to_action(idx: u32) -> ClickHoldAction {
    if idx == 1 {
        ClickHoldAction::Siri
    } else {
        ClickHoldAction::NoiseControl
    }
}

fn battery_group(state: &AirPodsState) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("Battery").build();
    group.add(&battery_row(state, "Left", "battery-left", "charging-left"));
    group.add(&battery_row(
        state,
        "Right",
        "battery-right",
        "charging-right",
    ));
    group.add(&battery_row(state, "Case", "battery-case", "charging-case"));
    group
}

fn battery_row(
    state: &AirPodsState,
    title: &str,
    level_prop: &str,
    charging_prop: &str,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(title).build();

    let suffix = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .valign(gtk::Align::Center)
        .build();

    let level_label = gtk::Label::builder().css_classes(["dim-label"]).build();
    state
        .bind_property(level_prop, &level_label, "label")
        .transform_to(|_, level: i32| {
            Some(
                if level < 0 {
                    "—".to_string()
                } else {
                    format!("{level}%")
                }
                .to_value(),
            )
        })
        .sync_create()
        .build();
    suffix.append(&level_label);

    let charging_icon = gtk::Image::from_icon_name("battery-good-charging-symbolic");
    state
        .bind_property(charging_prop, &charging_icon, "visible")
        .sync_create()
        .build();
    suffix.append(&charging_icon);

    row.add_suffix(&suffix);
    row
}

fn noise_control_group(state: &AirPodsState, commands: CommandSender) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Noise Control")
        .build();

    let row = adw::ActionRow::builder().title("Mode").build();

    let buttons = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(0)
        .valign(gtk::Align::Center)
        .css_classes(["linked"])
        .build();

    let off = anc_button("Off", ListeningMode::Off, &commands, None);
    let nc = anc_button(
        "Noise Cancellation",
        ListeningMode::NoiseCancellation,
        &commands,
        Some(&off),
    );
    let transparency = anc_button(
        "Transparency",
        ListeningMode::Transparency,
        &commands,
        Some(&off),
    );
    let adaptive = anc_button("Adaptive", ListeningMode::Adaptive, &commands, Some(&off));

    buttons.append(&off);
    buttons.append(&nc);
    buttons.append(&transparency);
    buttons.append(&adaptive);

    let by_mode = [
        (ListeningMode::Off as i32, off),
        (ListeningMode::NoiseCancellation as i32, nc),
        (ListeningMode::Transparency as i32, transparency),
        (ListeningMode::Adaptive as i32, adaptive),
    ];

    let sync = move |state: &AirPodsState| {
        let mode = state.listening_mode();
        for (m, btn) in &by_mode {
            btn.set_active(*m == mode);
        }
    };
    sync(state);
    state.connect_listening_mode_notify(sync);

    row.add_suffix(&buttons);
    group.add(&row);
    group
}

fn show_rename_dialog(parent: &adw::ApplicationWindow, commands: CommandSender) {
    let dialog = adw::MessageDialog::builder()
        .transient_for(parent)
        .modal(true)
        .heading("Rename AirPods")
        .body("New name will appear in your Bluetooth settings.")
        .default_response("rename")
        .close_response("cancel")
        .build();
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("rename", "Rename");
    dialog.set_response_appearance("rename", adw::ResponseAppearance::Suggested);

    let entry = gtk::Entry::builder()
        .placeholder_text("e.g. \"My AirPods Pro\"")
        .activates_default(true)
        .build();
    dialog.set_extra_child(Some(&entry));

    let entry_for_response = entry.clone();
    dialog.connect_response(None, move |dialog, response| {
        if response == "rename" {
            let name = entry_for_response.text().to_string();
            if !name.is_empty() {
                let commands = commands.clone();
                glib::spawn_future_local(async move {
                    if let Err(e) = commands.send(DaemonCommand::Rename(name)).await {
                        tracing::warn!("dropping Rename, daemon channel closed: {e}");
                    }
                });
            }
        }
        dialog.close();
    });

    dialog.present();
}

fn anc_button(
    label: &str,
    mode: ListeningMode,
    commands: &CommandSender,
    group: Option<&gtk::ToggleButton>,
) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::with_label(label);
    if let Some(other) = group {
        button.set_group(Some(other));
    }
    let commands = commands.clone();
    button.connect_clicked(move |btn| {
        if !btn.is_active() {
            return; // ignore the deselection edge of the toggle
        }
        let cmd = DaemonCommand::SetListeningMode(mode);
        let commands = commands.clone();
        glib::spawn_future_local(async move {
            if let Err(e) = commands.send(cmd).await {
                tracing::warn!("dropping ANC command, daemon channel closed: {e}");
            }
        });
    });
    button
}
