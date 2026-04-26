// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Pure domain model + pure event-application function.
//!
//! `AirPodsModel` is the single source of truth for "what we know about
//! the connected pair right now." `apply_event` is the only function
//! allowed to mutate it.
//!
//! Keeping the model and the mutator pure means:
//! - we can unit-test every state transition by hand-rolling
//!   `DaemonEvent` values, without GTK / tokio / bluez,
//! - the GObject layer (next slice) is a thin adapter on top — it
//!   listens to property changes from a `RefCell<AirPodsModel>` and
//!   notifies UI bindings, but it doesn't decide what changes.
//!
//! Anything richer than "remember the last value the AirPods told us"
//! belongs elsewhere: ear-detection-driven Pause/Play is in
//! `media_state`, gesture detection will be in `gesture`, etc.

use tragus_bluetooth::event::DaemonEvent;
use tragus_protocol::battery::{BatteryComponent, BatteryStatus};
use tragus_protocol::control_command::{ControlIdentifier, ListeningMode};
use tragus_protocol::ear_detection::EarStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryReading {
    pub level: u8,
    pub charging: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AirPodsModel {
    pub battery_left: Option<BatteryReading>,
    pub battery_right: Option<BatteryReading>,
    pub battery_case: Option<BatteryReading>,
    pub left_ear: Option<EarStatus>,
    pub right_ear: Option<EarStatus>,
    pub listening_mode: Option<ListeningMode>,
}

/// Mutate the model in response to one daemon event. Pure: no I/O,
/// no allocation beyond growing the model.
pub fn apply_event(model: &mut AirPodsModel, event: &DaemonEvent) {
    match event {
        DaemonEvent::Battery(notification) => {
            for entry in notification.entries() {
                let slot = match entry.component {
                    BatteryComponent::Left => &mut model.battery_left,
                    BatteryComponent::Right => &mut model.battery_right,
                    BatteryComponent::Case => &mut model.battery_case,
                };
                *slot = match entry.status {
                    BatteryStatus::Disconnected => None,
                    BatteryStatus::Charging => Some(BatteryReading {
                        level: entry.level,
                        charging: true,
                    }),
                    BatteryStatus::Discharging | BatteryStatus::Unknown => Some(BatteryReading {
                        level: entry.level,
                        charging: false,
                    }),
                };
            }
        }
        DaemonEvent::EarDetection(notification) => {
            // The protocol's "primary" / "secondary" labels swap when the
            // user takes the active pod out — we just mirror them into
            // left/right here. A later slice will resolve which physical
            // pod is which by reading per-bud serial numbers from
            // INFORMATION (opcode 0x1D).
            model.left_ear = Some(notification.primary);
            model.right_ear = Some(notification.secondary);
        }
        DaemonEvent::ControlCommand(cmd) => {
            if cmd.identifier == ControlIdentifier::ListeningMode
                && let Ok(mode) = ListeningMode::from_byte(cmd.data[0])
            {
                model.listening_mode = Some(mode);
            }
            // Other identifiers will land here as their UI screens arrive.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tragus_bluetooth::event::DaemonEvent;
    use tragus_protocol::battery::{
        BatteryComponent, BatteryEntry, BatteryNotification, BatteryStatus,
    };
    use tragus_protocol::control_command::{ControlCommand, ListeningMode};
    use tragus_protocol::ear_detection::{EarDetectionNotification, EarStatus};

    #[test]
    fn fresh_model_has_no_battery_or_ear_data() {
        let m = AirPodsModel::default();
        assert_eq!(m.battery_left, None);
        assert_eq!(m.battery_right, None);
        assert_eq!(m.battery_case, None);
        assert_eq!(m.left_ear, None);
        assert_eq!(m.right_ear, None);
        assert_eq!(m.listening_mode, None);
    }

    #[test]
    fn battery_event_populates_per_component_levels() {
        let mut m = AirPodsModel::default();
        let event = DaemonEvent::Battery(BatteryNotification::new(vec![
            BatteryEntry {
                component: BatteryComponent::Left,
                level: 80,
                status: BatteryStatus::Charging,
            },
            BatteryEntry {
                component: BatteryComponent::Right,
                level: 60,
                status: BatteryStatus::Discharging,
            },
            BatteryEntry {
                component: BatteryComponent::Case,
                level: 100,
                status: BatteryStatus::Charging,
            },
        ]));
        apply_event(&mut m, &event);

        assert_eq!(
            m.battery_left,
            Some(BatteryReading {
                level: 80,
                charging: true
            })
        );
        assert_eq!(
            m.battery_right,
            Some(BatteryReading {
                level: 60,
                charging: false
            })
        );
        assert_eq!(
            m.battery_case,
            Some(BatteryReading {
                level: 100,
                charging: true
            })
        );
    }

    #[test]
    fn battery_status_disconnected_clears_the_component() {
        let mut m = AirPodsModel {
            battery_left: Some(BatteryReading {
                level: 50,
                charging: false,
            }),
            ..Default::default()
        };

        let event = DaemonEvent::Battery(BatteryNotification::new(vec![BatteryEntry {
            component: BatteryComponent::Left,
            level: 0,
            status: BatteryStatus::Disconnected,
        }]));
        apply_event(&mut m, &event);

        assert_eq!(m.battery_left, None);
    }

    #[test]
    fn ear_detection_event_assigns_to_left_and_right() {
        // The protocol gives us (primary, secondary). We don't yet know
        // which physical pod is which — for the model we just remember the
        // primary slot in `left_ear` and the secondary in `right_ear`.
        // A later slice will resolve this against per-bud serial numbers
        // from opcode 0x1D.
        let mut m = AirPodsModel::default();
        let event = DaemonEvent::EarDetection(EarDetectionNotification {
            primary: EarStatus::InEar,
            secondary: EarStatus::InCase,
        });
        apply_event(&mut m, &event);

        assert_eq!(m.left_ear, Some(EarStatus::InEar));
        assert_eq!(m.right_ear, Some(EarStatus::InCase));
    }

    #[test]
    fn listening_mode_event_updates_anc() {
        let mut m = AirPodsModel::default();
        let event = DaemonEvent::ControlCommand(ControlCommand::set_listening_mode(
            ListeningMode::Adaptive,
        ));
        apply_event(&mut m, &event);

        assert_eq!(m.listening_mode, Some(ListeningMode::Adaptive));
    }

    #[test]
    fn unrelated_control_command_leaves_listening_mode_alone() {
        let mut m = AirPodsModel {
            listening_mode: Some(ListeningMode::Off),
            ..Default::default()
        };
        // Mic mode: identifier 0x01, not 0x0D.
        let event = DaemonEvent::ControlCommand(ControlCommand {
            identifier: tragus_protocol::control_command::ControlIdentifier::MicMode,
            data: [0x00, 0x00, 0x00, 0x00],
        });
        apply_event(&mut m, &event);

        assert_eq!(m.listening_mode, Some(ListeningMode::Off));
    }

    #[test]
    fn invalid_listening_mode_byte_is_ignored() {
        let mut m = AirPodsModel {
            listening_mode: Some(ListeningMode::NoiseCancellation),
            ..Default::default()
        };
        // 0x00 is not a valid ListeningMode (valid range is 1..=4).
        let event = DaemonEvent::ControlCommand(ControlCommand {
            identifier: tragus_protocol::control_command::ControlIdentifier::ListeningMode,
            data: [0x00, 0x00, 0x00, 0x00],
        });
        apply_event(&mut m, &event);

        // Previous value preserved — a malformed value is not a reason to
        // forget what we already knew.
        assert_eq!(m.listening_mode, Some(ListeningMode::NoiseCancellation));
    }
}
