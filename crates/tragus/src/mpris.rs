// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! MPRIS media-player control.
//!
//! Walks every `org.mpris.MediaPlayer2.*` bus name on the session bus
//! and:
//!
//! - on `Pause` — calls `Pause` on each player whose `PlaybackStatus`
//!   is `"Playing"`,
//! - on `Play` — calls `Play` on each player whose `PlaybackStatus`
//!   is `"Paused"`.
//!
//! The state machine in `media_state` decides *whether* to pause/play
//! based on ear-detection transitions; this module only translates a
//! decision into D-Bus calls.
//!
//! Errors are logged at `warn` and swallowed: a missing player or a
//! transient D-Bus glitch shouldn't crash the UI thread.

use zbus::Connection;
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::zvariant::Value;

const PLAYER_PREFIX: &str = "org.mpris.MediaPlayer2.";
const PLAYER_PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";

pub async fn pause_active_players() {
    if let Err(e) = act_on_players("Playing", "Pause").await {
        tracing::warn!("MPRIS pause failed: {e}");
    }
}

pub async fn play_paused_players() {
    if let Err(e) = act_on_players("Paused", "Play").await {
        tracing::warn!("MPRIS play failed: {e}");
    }
}

async fn act_on_players(target_status: &str, method: &str) -> zbus::Result<()> {
    let conn = Connection::session().await?;
    let dbus = DBusProxy::new(&conn).await?;

    for name in dbus.list_names().await? {
        let s = name.as_str();
        if !s.starts_with(PLAYER_PREFIX) {
            continue;
        }
        let bus_name = match BusName::try_from(s) {
            Ok(n) => n,
            Err(_) => continue,
        };
        if let Err(e) = act_on_one_player(&conn, &bus_name, target_status, method).await {
            tracing::debug!("MPRIS {method} on {s}: {e}");
        }
    }

    Ok(())
}

async fn act_on_one_player(
    conn: &Connection,
    bus: &BusName<'_>,
    target_status: &str,
    method: &str,
) -> zbus::Result<()> {
    let proxy = zbus::Proxy::new(conn, bus.clone(), PLAYER_PATH, PLAYER_IFACE).await?;
    let status: Value = proxy.get_property("PlaybackStatus").await?;
    let Value::Str(s) = status else {
        return Ok(());
    };
    if s.as_str() != target_status {
        return Ok(());
    }
    proxy.call_method(method, &()).await?;
    Ok(())
}
