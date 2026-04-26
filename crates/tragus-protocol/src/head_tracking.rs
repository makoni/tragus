// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

//! Head-tracking IMU parser (AAP opcode `0x17`).
//!
//! AirPods stream a ~25 Hz packet over the AAP socket once head
//! tracking is enabled. Each packet is around 70 bytes, but the
//! orientation + acceleration we care about live at fixed offsets
//! near the end. From the LibrePods Android `HeadOrientation.kt`
//! (offsets are byte indices into the **full** packet — including
//! the 6-byte AAP frame header):
//!
//! ```text
//! packet  payload  field
//! ------  -------  -----
//! 43–44   37–38    o1 (i16 LE) — primary orientation axis
//! 45–46   39–40    o2 (i16 LE) — secondary axis
//! 47–48   41–42    o3 (i16 LE) — tertiary axis
//! 51–52   45–46    horizontal acceleration (i16 LE)
//! 53–54   47–48    vertical acceleration (i16 LE)
//! ```
//!
//! Pitch / yaw are derived in the UI layer from these raw axes after
//! a 10-sample neutral baseline:
//!
//! ```text
//! pitch = (o2_norm + o3_norm) / 2 / 32000 * 180  (degrees)
//! yaw   = (o2_norm - o3_norm) / 2 / 32000 * 180
//! ```

use crate::error::ProtocolError;

/// AAP opcode for head-tracking IMU packets.
pub const OPCODE: u8 = 0x17;

const O1_OFFSET: usize = 37;
const HACCEL_OFFSET: usize = 45;
/// Smallest payload that can carry every field we read.
const MIN_PAYLOAD_LEN: usize = 49;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImuSample {
    pub o1: i16,
    pub o2: i16,
    pub o3: i16,
    pub horizontal_accel: i16,
    pub vertical_accel: i16,
}

impl ImuSample {
    pub fn parse(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < MIN_PAYLOAD_LEN {
            return Err(ProtocolError::TooShort {
                expected: MIN_PAYLOAD_LEN,
                got: payload.len(),
            });
        }
        Ok(Self {
            o1: read_i16(payload, O1_OFFSET),
            o2: read_i16(payload, O1_OFFSET + 2),
            o3: read_i16(payload, O1_OFFSET + 4),
            horizontal_accel: read_i16(payload, HACCEL_OFFSET),
            vertical_accel: read_i16(payload, HACCEL_OFFSET + 2),
        })
    }
}

fn read_i16(payload: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([payload[offset], payload[offset + 1]])
}

/// Compute pitch / yaw in degrees from the current sample relative to a
/// neutral-pose baseline.
///
/// The calling layer is responsible for averaging the first ~10 samples
/// into a baseline before feeding either real-time samples or a gesture
/// detector through here.
///
/// Formulas reproduced from LibrePods Android `HeadOrientation.kt`:
///
/// ```text
/// pitch = (o2_norm + o3_norm) / 2 / 32000 * 180   (degrees)
/// yaw   = (o2_norm - o3_norm) / 2 / 32000 * 180
/// ```
pub fn pitch_yaw_degrees(sample: &ImuSample, baseline: &ImuSample) -> (f32, f32) {
    let o2_norm = f32::from(sample.o2) - f32::from(baseline.o2);
    let o3_norm = f32::from(sample.o3) - f32::from(baseline.o3);
    let pitch = (o2_norm + o3_norm) / 2.0 / 32_000.0 * 180.0;
    let yaw = (o2_norm - o3_norm) / 2.0 / 32_000.0 * 180.0;
    (pitch, yaw)
}

#[cfg(test)]
mod tests {
    use crate::error::ProtocolError;
    use crate::head_tracking::{ImuSample, OPCODE};

    #[test]
    fn opcode_constant_matches_spec() {
        assert_eq!(OPCODE, 0x17);
    }

    #[test]
    fn parses_known_imu_values_from_synthetic_payload() {
        let mut payload = vec![0u8; 49];
        payload[37..39].copy_from_slice(&256_i16.to_le_bytes());
        payload[39..41].copy_from_slice(&(-1_i16).to_le_bytes());
        payload[41..43].copy_from_slice(&32000_i16.to_le_bytes());
        payload[45..47].copy_from_slice(&100_i16.to_le_bytes());
        payload[47..49].copy_from_slice(&(-200_i16).to_le_bytes());

        let s = ImuSample::parse(&payload).unwrap();
        assert_eq!(s.o1, 256);
        assert_eq!(s.o2, -1);
        assert_eq!(s.o3, 32000);
        assert_eq!(s.horizontal_accel, 100);
        assert_eq!(s.vertical_accel, -200);
    }

    #[test]
    fn payload_under_49_bytes_is_too_short() {
        let payload = vec![0u8; 30];
        assert!(matches!(
            ImuSample::parse(&payload),
            Err(ProtocolError::TooShort { .. }),
        ));
    }

    #[test]
    fn pitch_yaw_at_baseline_is_zero() {
        let baseline = ImuSample {
            o1: 100,
            o2: 200,
            o3: 300,
            horizontal_accel: 0,
            vertical_accel: 0,
        };
        let (pitch, yaw) = crate::head_tracking::pitch_yaw_degrees(&baseline, &baseline);
        assert_eq!(pitch, 0.0);
        assert_eq!(yaw, 0.0);
    }

    #[test]
    fn pitch_full_swing_when_axes_in_phase() {
        let baseline = ImuSample {
            o1: 0,
            o2: 0,
            o3: 0,
            horizontal_accel: 0,
            vertical_accel: 0,
        };
        let sample = ImuSample {
            o2: 32_000,
            o3: 32_000,
            ..baseline
        };
        let (pitch, yaw) = crate::head_tracking::pitch_yaw_degrees(&sample, &baseline);
        // (32000 + 32000) / 2 / 32000 * 180 == 180
        assert!((pitch - 180.0).abs() < 1e-3);
        assert_eq!(yaw, 0.0);
    }

    #[test]
    fn yaw_full_swing_when_axes_out_of_phase() {
        let baseline = ImuSample {
            o1: 0,
            o2: 0,
            o3: 0,
            horizontal_accel: 0,
            vertical_accel: 0,
        };
        let sample = ImuSample {
            o2: 32_000,
            o3: -32_000,
            ..baseline
        };
        let (pitch, yaw) = crate::head_tracking::pitch_yaw_degrees(&sample, &baseline);
        assert_eq!(pitch, 0.0);
        assert!((yaw - 180.0).abs() < 1e-3);
    }
}
