// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Glue between the daemon's `async_channel::Receiver<DaemonEvent>`
//! (filled from a tokio worker thread) and the `AirPodsState` GObject
//! that lives on the GTK main loop.
//!
//! `glib::spawn_future_local` runs the consumer task on the main loop's
//! executor — `async_channel` is rt-agnostic, so the same channel
//! works on both ends.
//!
//! No unit tests on purpose: this is a three-line adapter, and the
//! mutation it dispatches (`AirPodsState::apply_event`) is already
//! covered. Manual integration: run the app against a real socket or
//! the fake source, watch properties update.

use crate::state::AirPodsState;
use gtk::glib;
use tragus_bluetooth::event::DaemonEvent;

pub fn attach_event_stream(state: AirPodsState, events: async_channel::Receiver<DaemonEvent>) {
    glib::spawn_future_local(async move {
        while let Ok(event) = events.recv().await {
            state.apply_event(&event);
        }
    });
}
