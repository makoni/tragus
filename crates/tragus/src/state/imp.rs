// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! GObject internals for [`super::AirPodsState`].
//!
//! Holds a `RefCell<AirPodsModel>` plus one `Cell<bool>` for the
//! connection flag. Properties are derived through
//! `glib::Properties` with custom getters that project the model's
//! `Option<…>` fields onto i32-encoded values that bind cleanly to
//! `.ui` files (-1 means "unknown").

use std::cell::{Cell, RefCell};
use std::marker::PhantomData;

use gtk::glib;
use gtk::glib::Properties;
use gtk::glib::subclass::prelude::*;
use gtk::prelude::ObjectExt;
use tragus_bluetooth::event::DaemonEvent;
use tragus_protocol::ear_detection::EarStatus;

#[derive(Properties, Default)]
#[properties(wrapper_type = super::AirPodsState)]
pub struct AirPodsState {
    inner: RefCell<crate::model::AirPodsModel>,

    #[property(get = Self::battery_left)]
    _battery_left: PhantomData<i32>,
    #[property(get = Self::battery_right)]
    _battery_right: PhantomData<i32>,
    #[property(get = Self::battery_case)]
    _battery_case: PhantomData<i32>,

    #[property(get = Self::charging_left)]
    _charging_left: PhantomData<bool>,
    #[property(get = Self::charging_right)]
    _charging_right: PhantomData<bool>,
    #[property(get = Self::charging_case)]
    _charging_case: PhantomData<bool>,

    #[property(get = Self::listening_mode)]
    _listening_mode: PhantomData<i32>,

    #[property(get = Self::left_ear)]
    _left_ear: PhantomData<i32>,
    #[property(get = Self::right_ear)]
    _right_ear: PhantomData<i32>,

    #[property(get, set)]
    connected: Cell<bool>,
}

#[glib::object_subclass]
impl ObjectSubclass for AirPodsState {
    const NAME: &'static str = "TragusAirPodsState";
    type Type = super::AirPodsState;
}

#[glib::derived_properties]
impl ObjectImpl for AirPodsState {}

impl AirPodsState {
    fn battery_left(&self) -> i32 {
        self.inner
            .borrow()
            .battery_left
            .map_or(-1, |b| i32::from(b.level))
    }
    fn battery_right(&self) -> i32 {
        self.inner
            .borrow()
            .battery_right
            .map_or(-1, |b| i32::from(b.level))
    }
    fn battery_case(&self) -> i32 {
        self.inner
            .borrow()
            .battery_case
            .map_or(-1, |b| i32::from(b.level))
    }

    fn charging_left(&self) -> bool {
        self.inner.borrow().battery_left.is_some_and(|b| b.charging)
    }
    fn charging_right(&self) -> bool {
        self.inner
            .borrow()
            .battery_right
            .is_some_and(|b| b.charging)
    }
    fn charging_case(&self) -> bool {
        self.inner.borrow().battery_case.is_some_and(|b| b.charging)
    }

    fn listening_mode(&self) -> i32 {
        self.inner
            .borrow()
            .listening_mode
            .map_or(-1, |m| i32::from(m as u8))
    }

    fn left_ear(&self) -> i32 {
        self.inner.borrow().left_ear.map_or(-1, ear_status_to_i32)
    }
    fn right_ear(&self) -> i32 {
        self.inner.borrow().right_ear.map_or(-1, ear_status_to_i32)
    }

    pub fn apply_event(&self, event: &DaemonEvent) {
        crate::model::apply_event(&mut self.inner.borrow_mut(), event);
        let obj = self.obj();
        match event {
            DaemonEvent::Battery(_) => {
                obj.notify_battery_left();
                obj.notify_charging_left();
                obj.notify_battery_right();
                obj.notify_charging_right();
                obj.notify_battery_case();
                obj.notify_charging_case();
            }
            DaemonEvent::EarDetection(_) => {
                obj.notify_left_ear();
                obj.notify_right_ear();
            }
            DaemonEvent::ControlCommand(_) => {
                obj.notify_listening_mode();
            }
            DaemonEvent::HeadTracking(_) => {
                // Not stored on the AirPodsState — see model.rs for the
                // rationale. Future: emit a 'head-tracking-sample' signal
                // for visualisation widgets to subscribe to.
            }
        }
    }
}

fn ear_status_to_i32(status: EarStatus) -> i32 {
    match status {
        EarStatus::InEar => 0,
        EarStatus::OutOfEar => 1,
        EarStatus::InCase => 2,
    }
}
