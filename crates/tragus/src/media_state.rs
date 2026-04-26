// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

//! Pure ear-detection → media-control state machine.
//!
//! Inputs are `EarStatus` events from the AirPods (one per in/out
//! transition), outputs are `MediaCommand`s for whatever player driver
//! sits above this — MPRIS in production, a `Vec` in tests.
//!
//! Three policies, mirroring the LibrePods Android client:
//!
//! - `PauseWhenOneRemoved` — strictest. Pause when either pod leaves
//!   the ear; resume only when both are back in.
//! - `PauseWhenBothRemoved` — looser. Pause only when both leave the
//!   ear; resume as soon as either is back in.
//! - `Disabled` — never emits.
//!
//! ## We-paused tracking
//!
//! `we_paused` is on whenever this module emitted a `Pause`. It gates
//! the resume side: if the user paused manually (we never set
//! `we_paused`), putting pods back in must not auto-play.
//!
//! What this module deliberately does **not** model:
//!
//! - User play/pause via MPRIS while pods are out (we'd need MPRIS
//!   subscription; that comes in the bridge layer).
//! - Conversational-awareness ducking (separate concern).

use tragus_protocol::ear_detection::EarStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "PauseWhenBothRemoved / Disabled get UI selectors in M3.I"
)]
pub enum EarDetectionPolicy {
    PauseWhenOneRemoved,
    PauseWhenBothRemoved,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCommand {
    Pause,
    Play,
}

#[derive(Debug)]
pub struct MediaState {
    policy: EarDetectionPolicy,
    we_paused: bool,
    last: Option<(EarStatus, EarStatus)>,
}

impl MediaState {
    pub fn new(policy: EarDetectionPolicy) -> Self {
        Self {
            policy,
            we_paused: false,
            last: None,
        }
    }

    /// Feed a fresh ear-detection notification. Returns `Some(command)`
    /// only when the transition warrants action, taking the policy and
    /// our own pause-tracking into account.
    pub fn on_ear(&mut self, primary: EarStatus, secondary: EarStatus) -> Option<MediaCommand> {
        let new_pair = (primary, secondary);
        let old_pair = self.last;
        self.last = Some(new_pair);

        if matches!(self.policy, EarDetectionPolicy::Disabled) {
            return None;
        }

        // First event ever — establish baseline, no command.
        let old = old_pair?;

        let was_listening = is_listening(old, self.policy);
        let now_listening = is_listening(new_pair, self.policy);

        match (was_listening, now_listening) {
            (true, false) => {
                self.we_paused = true;
                Some(MediaCommand::Pause)
            }
            (false, true) if self.we_paused => {
                self.we_paused = false;
                Some(MediaCommand::Play)
            }
            _ => None,
        }
    }
}

/// "Listening" means the user is currently wearing the pods according
/// to the active policy.
fn is_listening(pair: (EarStatus, EarStatus), policy: EarDetectionPolicy) -> bool {
    let (p, s) = pair;
    match policy {
        EarDetectionPolicy::PauseWhenOneRemoved => p == EarStatus::InEar && s == EarStatus::InEar,
        EarDetectionPolicy::PauseWhenBothRemoved => p == EarStatus::InEar || s == EarStatus::InEar,
        EarDetectionPolicy::Disabled => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tragus_protocol::ear_detection::EarStatus;

    #[test]
    fn first_event_is_baseline_no_command() {
        let mut m = MediaState::new(EarDetectionPolicy::PauseWhenOneRemoved);
        assert_eq!(m.on_ear(EarStatus::InEar, EarStatus::InEar), None);
    }

    #[test]
    fn pause_when_one_removed_round_trip() {
        let mut m = MediaState::new(EarDetectionPolicy::PauseWhenOneRemoved);
        m.on_ear(EarStatus::InEar, EarStatus::InEar);

        assert_eq!(
            m.on_ear(EarStatus::InEar, EarStatus::OutOfEar),
            Some(MediaCommand::Pause),
        );
        assert_eq!(
            m.on_ear(EarStatus::InEar, EarStatus::InEar),
            Some(MediaCommand::Play),
        );
        // Same state again → no duplicate Play.
        assert_eq!(m.on_ear(EarStatus::InEar, EarStatus::InEar), None);
    }

    #[test]
    fn pause_when_both_removed_only_acts_when_both_out() {
        let mut m = MediaState::new(EarDetectionPolicy::PauseWhenBothRemoved);
        m.on_ear(EarStatus::InEar, EarStatus::InEar);

        // One removed — under this policy, no Pause yet.
        assert_eq!(m.on_ear(EarStatus::InEar, EarStatus::OutOfEar), None);

        // Both removed — Pause.
        assert_eq!(
            m.on_ear(EarStatus::OutOfEar, EarStatus::OutOfEar),
            Some(MediaCommand::Pause),
        );

        // Putting one back is enough to resume.
        assert_eq!(
            m.on_ear(EarStatus::InEar, EarStatus::OutOfEar),
            Some(MediaCommand::Play),
        );
    }

    #[test]
    fn disabled_policy_never_emits() {
        let mut m = MediaState::new(EarDetectionPolicy::Disabled);
        m.on_ear(EarStatus::InEar, EarStatus::InEar);
        assert_eq!(m.on_ear(EarStatus::OutOfEar, EarStatus::OutOfEar), None);
        assert_eq!(m.on_ear(EarStatus::InEar, EarStatus::InEar), None);
    }

    #[test]
    fn does_not_resume_if_we_did_not_pause() {
        // Pods come out of the case directly into the ear (no prior pause).
        // Going from in-case to in-ear should not emit Play, because we
        // never asked anyone to pause.
        let mut m = MediaState::new(EarDetectionPolicy::PauseWhenOneRemoved);
        m.on_ear(EarStatus::InCase, EarStatus::InCase);
        assert_eq!(m.on_ear(EarStatus::InEar, EarStatus::InEar), None);
    }

    #[test]
    fn in_case_treated_as_out_of_ear_for_pause_one() {
        let mut m = MediaState::new(EarDetectionPolicy::PauseWhenOneRemoved);
        m.on_ear(EarStatus::InEar, EarStatus::InEar);
        // Putting one in the case (e.g. dropped it back) should pause.
        assert_eq!(
            m.on_ear(EarStatus::InEar, EarStatus::InCase),
            Some(MediaCommand::Pause),
        );
    }
}
