// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Routing layer between raw `OwnedFrame`s and typed `DaemonEvent`s.
//!
//! This is the seam between bytes and the rest of the application: every
//! frame we read off the L2CAP socket goes through `DaemonEvent::from_frame`
//! exactly once. Adding a new opcode means: write the parser in
//! `tragus-protocol`, add a variant here, route it.
//!
//! Pure function, no I/O — fully unit-tested.

use tragus_protocol::ProtocolError;
use tragus_protocol::battery::{self, BatteryNotification};
use tragus_protocol::control_command::{self, ControlCommand};
use tragus_protocol::ear_detection::{self, EarDetectionNotification};
use tragus_protocol::frame::OwnedFrame;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonEvent {
    Battery(BatteryNotification),
    EarDetection(EarDetectionNotification),
    ControlCommand(ControlCommand),
}

impl DaemonEvent {
    /// Convert a frame received from the AirPods into a typed event.
    ///
    /// - `Ok(Some(event))` — opcode is one we handle; payload parsed cleanly.
    /// - `Ok(None)` — opcode is unknown to us. Caller can log or surface
    ///   it in a debug view, but should not treat it as an error: Apple
    ///   adds opcodes silently and most are harmless.
    /// - `Err(_)` — opcode is one we know but the payload was malformed.
    ///   This is worth surfacing.
    pub fn from_frame(frame: &OwnedFrame) -> Result<Option<Self>, ProtocolError> {
        match frame.opcode {
            battery::OPCODE => {
                BatteryNotification::parse(&frame.payload).map(|b| Some(Self::Battery(b)))
            }
            ear_detection::OPCODE => {
                EarDetectionNotification::parse(&frame.payload).map(|e| Some(Self::EarDetection(e)))
            }
            control_command::OPCODE => {
                ControlCommand::parse(&frame.payload).map(|c| Some(Self::ControlCommand(c)))
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::event::DaemonEvent;
    use tragus_protocol::OwnedFrame;
    use tragus_protocol::battery::{BatteryComponent, BatteryStatus};
    use tragus_protocol::control_command::{ControlIdentifier, ListeningMode};
    use tragus_protocol::ear_detection::EarStatus;

    fn frame(opcode: u8, payload: &[u8]) -> OwnedFrame {
        OwnedFrame {
            opcode,
            payload: payload.to_vec(),
        }
    }

    #[test]
    fn battery_frame_dispatches_to_battery_event() {
        let f = frame(
            0x04,
            &[
                0x03, // 3 entries
                0x02, 0x01, 0x64, 0x02, 0x01, // Right, 100%, Discharging
                0x04, 0x01, 0x63, 0x01, 0x01, // Left,  99%, Charging
                0x08, 0x01, 0x11, 0x02, 0x01, // Case,  17%, Discharging
            ],
        );

        let DaemonEvent::Battery(battery) = DaemonEvent::from_frame(&f).unwrap().unwrap() else {
            panic!("expected Battery event");
        };
        let entries = battery.entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[1].component, BatteryComponent::Left);
        assert_eq!(entries[1].level, 99);
        assert_eq!(entries[1].status, BatteryStatus::Charging);
    }

    #[test]
    fn ear_detection_frame_dispatches() {
        let f = frame(0x06, &[0x00, 0x01]); // primary InEar, secondary OutOfEar
        let DaemonEvent::EarDetection(ear) = DaemonEvent::from_frame(&f).unwrap().unwrap() else {
            panic!("expected EarDetection event");
        };
        assert_eq!(ear.primary, EarStatus::InEar);
        assert_eq!(ear.secondary, EarStatus::OutOfEar);
    }

    #[test]
    fn control_command_listening_mode_dispatches() {
        let f = frame(0x09, &[0x0D, 0x02, 0x00, 0x00, 0x00]); // ListeningMode = NoiseCancellation
        let DaemonEvent::ControlCommand(cmd) = DaemonEvent::from_frame(&f).unwrap().unwrap() else {
            panic!("expected ControlCommand event");
        };
        assert_eq!(cmd.identifier, ControlIdentifier::ListeningMode);
        assert_eq!(
            ListeningMode::from_byte(cmd.data[0]).unwrap(),
            ListeningMode::NoiseCancellation,
        );
    }

    #[test]
    fn unknown_opcode_dispatches_to_none() {
        let f = frame(0xFF, &[0xAA, 0xBB, 0xCC]);
        assert!(DaemonEvent::from_frame(&f).unwrap().is_none());
    }

    #[test]
    fn malformed_battery_payload_surfaces_protocol_error() {
        let f = frame(0x04, &[0x05, 0x00]); // claims 5 entries, has none
        assert!(DaemonEvent::from_frame(&f).is_err());
    }
}
