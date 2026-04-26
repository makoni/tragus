// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

//! Control commands (AAP opcode `0x09`).
//!
//! All control commands share a fixed 5-byte payload:
//!
//! ```text
//! identifier : u8
//! data       : [u8; 4]   // unused bytes are 0x00
//! ```
//!
//! See `docs/control_commands.md` for the full identifier table. Most
//! identifiers use only `data[0]`; a few use `data[1]` for per-bud values
//! (e.g. `ClickHoldMode` carries right then left) or for two-state
//! features (e.g. `HearingAid` carries enrolled then enabled).
//!
//! Identifier-specific value enums (`ListeningMode`, `EnabledDisabled`)
//! live here too. We add new typed value enums as the higher layers need
//! them — callers can always fall back to the raw `data` bytes.

use crate::error::ProtocolError;

/// AAP opcode for control commands.
pub const OPCODE: u8 = 0x09;

/// Length of the payload after the AAP frame header.
pub const PAYLOAD_LEN: usize = 5;

/// Every control-command identifier the spec documents.
///
/// `Unknown(u8)` carries any byte the firmware sends that we don't have a
/// name for yet — we never error on an unknown identifier, since the
/// AirPods firmware is the moving target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlIdentifier {
    MicMode,
    ButtonSendMode,
    OwnsConnection,
    EarDetectionEnabled,
    ListeningMode,
    VoiceTrigger,
    SingleClickMode,
    DoubleClickMode,
    ClickHoldMode,
    DoubleClickInterval,
    ClickHoldInterval,
    ListeningModeConfigs,
    OneBudAncMode,
    CrownRotationDirection,
    AutoAnswerMode,
    ChimeVolume,
    ConnectAutomatically,
    VolumeSwipeInterval,
    CallManagementConfig,
    VolumeSwipeMode,
    AdaptiveVolumeConfig,
    SoftwareMuteConfig,
    ConversationDetectConfig,
    Ssl,
    HearingAid,
    AutoAncStrength,
    HpsGainSwipe,
    HrmState,
    InCaseToneConfig,
    SiriMultitoneConfig,
    HearingAssistConfig,
    AllowOffOption,
    SleepDetectionConfig,
    AllowAutoConnect,
    PpeToggleConfig,
    PpeCapLevelConfig,
    RawGesturesConfig,
    TemporaryPairingConfig,
    DynamicEndOfChargeConfig,
    SystemSiriMessageConfig,
    HearingAidGenericConfig,
    UplinkEqBudConfig,
    UplinkEqSourceConfig,
    InCaseToneVolume,
    DisableButtonInputConfig,
    Unknown(u8),
}

impl ControlIdentifier {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0x01 => Self::MicMode,
            0x05 => Self::ButtonSendMode,
            0x06 => Self::OwnsConnection,
            0x0A => Self::EarDetectionEnabled,
            0x0D => Self::ListeningMode,
            0x12 => Self::VoiceTrigger,
            0x14 => Self::SingleClickMode,
            0x15 => Self::DoubleClickMode,
            0x16 => Self::ClickHoldMode,
            0x17 => Self::DoubleClickInterval,
            0x18 => Self::ClickHoldInterval,
            0x1A => Self::ListeningModeConfigs,
            0x1B => Self::OneBudAncMode,
            0x1C => Self::CrownRotationDirection,
            0x1E => Self::AutoAnswerMode,
            0x1F => Self::ChimeVolume,
            0x20 => Self::ConnectAutomatically,
            0x23 => Self::VolumeSwipeInterval,
            0x24 => Self::CallManagementConfig,
            0x25 => Self::VolumeSwipeMode,
            0x26 => Self::AdaptiveVolumeConfig,
            0x27 => Self::SoftwareMuteConfig,
            0x28 => Self::ConversationDetectConfig,
            0x29 => Self::Ssl,
            0x2C => Self::HearingAid,
            0x2E => Self::AutoAncStrength,
            0x2F => Self::HpsGainSwipe,
            0x30 => Self::HrmState,
            0x31 => Self::InCaseToneConfig,
            0x32 => Self::SiriMultitoneConfig,
            0x33 => Self::HearingAssistConfig,
            0x34 => Self::AllowOffOption,
            0x35 => Self::SleepDetectionConfig,
            0x36 => Self::AllowAutoConnect,
            0x37 => Self::PpeToggleConfig,
            0x38 => Self::PpeCapLevelConfig,
            0x39 => Self::RawGesturesConfig,
            0x3A => Self::TemporaryPairingConfig,
            0x3B => Self::DynamicEndOfChargeConfig,
            0x3C => Self::SystemSiriMessageConfig,
            0x3D => Self::HearingAidGenericConfig,
            0x3E => Self::UplinkEqBudConfig,
            0x3F => Self::UplinkEqSourceConfig,
            0x40 => Self::InCaseToneVolume,
            0x41 => Self::DisableButtonInputConfig,
            other => Self::Unknown(other),
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            Self::MicMode => 0x01,
            Self::ButtonSendMode => 0x05,
            Self::OwnsConnection => 0x06,
            Self::EarDetectionEnabled => 0x0A,
            Self::ListeningMode => 0x0D,
            Self::VoiceTrigger => 0x12,
            Self::SingleClickMode => 0x14,
            Self::DoubleClickMode => 0x15,
            Self::ClickHoldMode => 0x16,
            Self::DoubleClickInterval => 0x17,
            Self::ClickHoldInterval => 0x18,
            Self::ListeningModeConfigs => 0x1A,
            Self::OneBudAncMode => 0x1B,
            Self::CrownRotationDirection => 0x1C,
            Self::AutoAnswerMode => 0x1E,
            Self::ChimeVolume => 0x1F,
            Self::ConnectAutomatically => 0x20,
            Self::VolumeSwipeInterval => 0x23,
            Self::CallManagementConfig => 0x24,
            Self::VolumeSwipeMode => 0x25,
            Self::AdaptiveVolumeConfig => 0x26,
            Self::SoftwareMuteConfig => 0x27,
            Self::ConversationDetectConfig => 0x28,
            Self::Ssl => 0x29,
            Self::HearingAid => 0x2C,
            Self::AutoAncStrength => 0x2E,
            Self::HpsGainSwipe => 0x2F,
            Self::HrmState => 0x30,
            Self::InCaseToneConfig => 0x31,
            Self::SiriMultitoneConfig => 0x32,
            Self::HearingAssistConfig => 0x33,
            Self::AllowOffOption => 0x34,
            Self::SleepDetectionConfig => 0x35,
            Self::AllowAutoConnect => 0x36,
            Self::PpeToggleConfig => 0x37,
            Self::PpeCapLevelConfig => 0x38,
            Self::RawGesturesConfig => 0x39,
            Self::TemporaryPairingConfig => 0x3A,
            Self::DynamicEndOfChargeConfig => 0x3B,
            Self::SystemSiriMessageConfig => 0x3C,
            Self::HearingAidGenericConfig => 0x3D,
            Self::UplinkEqBudConfig => 0x3E,
            Self::UplinkEqSourceConfig => 0x3F,
            Self::InCaseToneVolume => 0x40,
            Self::DisableButtonInputConfig => 0x41,
            Self::Unknown(b) => b,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListeningMode {
    Off = 0x01,
    NoiseCancellation = 0x02,
    Transparency = 0x03,
    Adaptive = 0x04,
}

impl ListeningMode {
    pub fn from_byte(b: u8) -> Result<Self, ProtocolError> {
        match b {
            0x01 => Ok(Self::Off),
            0x02 => Ok(Self::NoiseCancellation),
            0x03 => Ok(Self::Transparency),
            0x04 => Ok(Self::Adaptive),
            other => Err(ProtocolError::InvalidValue {
                what: "listening mode",
                byte: other,
            }),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnabledDisabled {
    Enabled = 0x01,
    Disabled = 0x02,
}

impl EnabledDisabled {
    pub fn from_byte(b: u8) -> Result<Self, ProtocolError> {
        match b {
            0x01 => Ok(Self::Enabled),
            0x02 => Ok(Self::Disabled),
            other => Err(ProtocolError::InvalidValue {
                what: "enabled/disabled",
                byte: other,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlCommand {
    pub identifier: ControlIdentifier,
    pub data: [u8; 4],
}

impl ControlCommand {
    pub fn parse(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < PAYLOAD_LEN {
            return Err(ProtocolError::TooShort {
                expected: PAYLOAD_LEN,
                got: payload.len(),
            });
        }
        Ok(Self {
            identifier: ControlIdentifier::from_byte(payload[0]),
            data: [payload[1], payload[2], payload[3], payload[4]],
        })
    }

    pub fn encode_payload(&self) -> [u8; PAYLOAD_LEN] {
        [
            self.identifier.to_byte(),
            self.data[0],
            self.data[1],
            self.data[2],
            self.data[3],
        ]
    }

    /// Build a typed `ListeningMode` switch command.
    pub fn set_listening_mode(mode: ListeningMode) -> Self {
        Self {
            identifier: ControlIdentifier::ListeningMode,
            data: [mode as u8, 0, 0, 0],
        }
    }

    /// Build an enable/disable command for any toggle-style identifier.
    pub fn set_toggle(identifier: ControlIdentifier, value: EnabledDisabled) -> Self {
        Self {
            identifier,
            data: [value as u8, 0, 0, 0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_roundtrip_for_every_byte() {
        // Round-tripping every possible byte proves both that `from_byte`
        // is a complete inverse of `to_byte` for every named variant and
        // that `Unknown(b)` carries the byte through losslessly.
        for byte in 0..=0xFFu8 {
            assert_eq!(ControlIdentifier::from_byte(byte).to_byte(), byte);
        }
    }

    #[test]
    fn parses_listening_mode_anc() {
        // 04 00 04 00 09 00 0D 02 00 00 00  →  ListeningMode = NoiseCancellation
        let payload = [0x0D, 0x02, 0x00, 0x00, 0x00];
        let cmd = ControlCommand::parse(&payload).unwrap();
        assert_eq!(cmd.identifier, ControlIdentifier::ListeningMode);
        assert_eq!(cmd.data, [0x02, 0x00, 0x00, 0x00]);
        assert_eq!(
            ListeningMode::from_byte(cmd.data[0]).unwrap(),
            ListeningMode::NoiseCancellation,
        );
    }

    #[test]
    fn parses_click_hold_mode_with_two_values() {
        // 0x16 ClickHoldMode: data[0] = right (NC=0x01), data[1] = left (Siri=0x05)
        let payload = [0x16, 0x01, 0x05, 0x00, 0x00];
        let cmd = ControlCommand::parse(&payload).unwrap();
        assert_eq!(cmd.identifier, ControlIdentifier::ClickHoldMode);
        assert_eq!(cmd.data[0], 0x01);
        assert_eq!(cmd.data[1], 0x05);
    }

    #[test]
    fn unknown_identifier_preserved() {
        let payload = [0xFE, 0xAB, 0xCD, 0xEF, 0x12];
        let cmd = ControlCommand::parse(&payload).unwrap();
        assert_eq!(cmd.identifier, ControlIdentifier::Unknown(0xFE));
        assert_eq!(cmd.encode_payload(), payload);
    }

    #[test]
    fn payload_too_short() {
        assert_eq!(
            ControlCommand::parse(&[0x0D, 0x02]),
            Err(ProtocolError::TooShort {
                expected: 5,
                got: 2,
            }),
        );
    }

    #[test]
    fn set_listening_mode_builder() {
        let cmd = ControlCommand::set_listening_mode(ListeningMode::Adaptive);
        assert_eq!(cmd.identifier, ControlIdentifier::ListeningMode);
        assert_eq!(cmd.encode_payload(), [0x0D, 0x04, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn set_toggle_builder() {
        let cmd = ControlCommand::set_toggle(
            ControlIdentifier::EarDetectionEnabled,
            EnabledDisabled::Disabled,
        );
        assert_eq!(cmd.encode_payload(), [0x0A, 0x02, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn listening_mode_invalid_byte() {
        assert_eq!(
            ListeningMode::from_byte(0x00),
            Err(ProtocolError::InvalidValue {
                what: "listening mode",
                byte: 0x00,
            }),
        );
        assert_eq!(
            ListeningMode::from_byte(0x05),
            Err(ProtocolError::InvalidValue {
                what: "listening mode",
                byte: 0x05,
            }),
        );
    }

    #[test]
    fn enabled_disabled_invalid_byte() {
        assert_eq!(
            EnabledDisabled::from_byte(0x00),
            Err(ProtocolError::InvalidValue {
                what: "enabled/disabled",
                byte: 0x00,
            }),
        );
    }

    #[test]
    fn encode_then_parse_roundtrip() {
        let original = ControlCommand {
            identifier: ControlIdentifier::ChimeVolume,
            data: [50, 0, 0, 0],
        };
        let encoded = original.encode_payload();
        assert_eq!(ControlCommand::parse(&encoded).unwrap(), original);
    }
}
