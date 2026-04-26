// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Synthetic event source for developing UI without AirPods nearby.
//!
//! Triggered by passing `--fake` on the command line (or setting
//! `TRAGUS_FAKE=1`). The function pushes a one-off batch of
//! representative events onto the same channel the real daemon would
//! use, then cycles through the four ANC modes every few seconds so
//! property bindings have something visible to react to.
//!
//! Strictly a dev tool — never wire this in alongside a real daemon.

use gtk::glib;
use tragus_bluetooth::event::DaemonEvent;
use tragus_protocol::battery::{
    BatteryComponent, BatteryEntry, BatteryNotification, BatteryStatus,
};
use tragus_protocol::control_command::{ControlCommand, ListeningMode};
use tragus_protocol::ear_detection::{EarDetectionNotification, EarStatus};

const ANC_CYCLE: [ListeningMode; 4] = [
    ListeningMode::Off,
    ListeningMode::NoiseCancellation,
    ListeningMode::Transparency,
    ListeningMode::Adaptive,
];

pub fn spawn_fake_source(events: async_channel::Sender<DaemonEvent>) {
    glib::spawn_future_local(async move {
        // First snapshot — what the user sees on app launch.
        if events.send(initial_battery()).await.is_err() {
            return;
        }
        if events.send(initial_ear_detection()).await.is_err() {
            return;
        }
        if events.send(initial_listening_mode()).await.is_err() {
            return;
        }

        // Slowly cycle ANC modes so anything bound to listening_mode
        // demonstrably updates. Three seconds is slow enough to read.
        let mut i: usize = 0;
        loop {
            glib::timeout_future_seconds(3).await;
            i = i.wrapping_add(1);
            let mode = ANC_CYCLE[i % ANC_CYCLE.len()];
            let event = DaemonEvent::ControlCommand(ControlCommand::set_listening_mode(mode));
            if events.send(event).await.is_err() {
                return;
            }
        }
    });
}

fn initial_battery() -> DaemonEvent {
    DaemonEvent::Battery(BatteryNotification::new(vec![
        BatteryEntry {
            component: BatteryComponent::Left,
            level: 80,
            status: BatteryStatus::Discharging,
        },
        BatteryEntry {
            component: BatteryComponent::Right,
            level: 75,
            status: BatteryStatus::Discharging,
        },
        BatteryEntry {
            component: BatteryComponent::Case,
            level: 100,
            status: BatteryStatus::Charging,
        },
    ]))
}

fn initial_ear_detection() -> DaemonEvent {
    DaemonEvent::EarDetection(EarDetectionNotification {
        primary: EarStatus::InEar,
        secondary: EarStatus::InEar,
    })
}

fn initial_listening_mode() -> DaemonEvent {
    DaemonEvent::ControlCommand(ControlCommand::set_listening_mode(
        ListeningMode::NoiseCancellation,
    ))
}
