// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! `AirPodsState` — GObject the UI binds to.
//!
//! Internals (`imp::AirPodsState`) own a `RefCell<AirPodsModel>`. This
//! wrapper is the public face: callers construct it, hand it events,
//! and read i32/bool properties for binding into `.ui` files.
//!
//! Why i32 for batteries / modes / ear-status: GLib property bindings
//! and Adwaita templates work much better with simple primitive types
//! than with `Option<…>` enums. We pick `-1` as a sentinel for "unknown
//! yet" and document it in the imp.

mod imp;

use gtk::glib;
use gtk::glib::subclass::prelude::*;
use tragus_bluetooth::event::DaemonEvent;

glib::wrapper! {
    pub struct AirPodsState(ObjectSubclass<imp::AirPodsState>);
}

impl AirPodsState {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn apply_event(&self, event: &DaemonEvent) {
        self.imp().apply_event(event);
    }
}

impl Default for AirPodsState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::state::AirPodsState;
    use tragus_bluetooth::event::DaemonEvent;
    use tragus_protocol::battery::{
        BatteryComponent, BatteryEntry, BatteryNotification, BatteryStatus,
    };
    use tragus_protocol::control_command::{ControlCommand, ListeningMode};

    fn battery_event(component: BatteryComponent, level: u8, charging: bool) -> DaemonEvent {
        DaemonEvent::Battery(BatteryNotification::new(vec![BatteryEntry {
            component,
            level,
            status: if charging {
                BatteryStatus::Charging
            } else {
                BatteryStatus::Discharging
            },
        }]))
    }

    #[test]
    fn fresh_state_reports_unknown_for_every_battery() {
        let state = AirPodsState::new();
        assert_eq!(state.battery_left(), -1);
        assert_eq!(state.battery_right(), -1);
        assert_eq!(state.battery_case(), -1);
        assert!(!state.charging_left());
        assert!(!state.charging_right());
        assert!(!state.charging_case());
        assert_eq!(state.listening_mode(), -1);
        assert!(!state.connected());
    }

    #[test]
    fn battery_event_drives_left_properties() {
        let state = AirPodsState::new();
        state.apply_event(&battery_event(BatteryComponent::Left, 80, true));
        assert_eq!(state.battery_left(), 80);
        assert!(state.charging_left());
    }

    #[test]
    fn listening_mode_event_drives_anc_property() {
        let state = AirPodsState::new();
        state.apply_event(&DaemonEvent::ControlCommand(
            ControlCommand::set_listening_mode(ListeningMode::Adaptive),
        ));
        assert_eq!(state.listening_mode(), 0x04);
    }

    #[test]
    fn set_connected_emits_property_change() {
        let state = AirPodsState::new();
        assert!(!state.connected());
        state.set_connected(true);
        assert!(state.connected());
    }
}
