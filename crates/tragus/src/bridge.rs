// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Glue between the daemon's `async_channel::Receiver<DaemonEvent>`
//! (filled from a tokio worker thread) and:
//!
//! - the `AirPodsState` GObject that lives on the GTK main loop, and
//! - the `MediaState` ear-detection state machine that translates
//!   `EarDetection` events into MPRIS pause/play calls.
//!
//! `glib::spawn_future_local` runs the consumer task on the main loop's
//! executor — `async_channel` is rt-agnostic, so the same channel
//! works on both ends.
//!
//! No unit tests on purpose: the mutation it dispatches is covered in
//! `model::apply_event` and `media_state::on_ear`. The MPRIS half is
//! integration-only.

use crate::media_state::{EarDetectionPolicy, MediaCommand, MediaState};
use crate::mpris;
use crate::state::AirPodsState;
use gtk::glib;
use tragus_bluetooth::event::DaemonEvent;

pub fn attach_event_stream(state: AirPodsState, events: async_channel::Receiver<DaemonEvent>) {
    let mut media = MediaState::new(EarDetectionPolicy::PauseWhenOneRemoved);
    glib::spawn_future_local(async move {
        while let Ok(event) = events.recv().await {
            tracing::debug!(?event, "bridge: applying event to AirPodsState");
            // Update the UI state first so widgets refresh promptly,
            // even if the MPRIS calls below take a moment.
            state.apply_event(&event);

            if let DaemonEvent::EarDetection(notification) = &event
                && let Some(cmd) = media.on_ear(notification.primary, notification.secondary)
            {
                tracing::info!(?cmd, "ear detection → MPRIS");
                match cmd {
                    MediaCommand::Pause => mpris::pause_active_players().await,
                    MediaCommand::Play => mpris::play_paused_players().await,
                }
            }
        }
        tracing::debug!("event channel closed; bridge task exiting");
    });
}
