// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Read-loop half of the daemon.
//!
//! Drives an `AsyncRead` (the L2CAP socket from `bluer` in production,
//! a `tokio_test::io::Builder` mock in tests), parses each incoming
//! frame into a [`DaemonEvent`], and forwards it onto an
//! `async_channel::Sender`.
//!
//! Why `async_channel` rather than `tokio::sync::mpsc`: the receiver
//! needs to be polled from inside the GTK main loop via
//! `glib::spawn_future_local`, where a tokio runtime isn't necessarily
//! the executor. `async_channel` is rt-agnostic.
//!
//! The loop returns:
//! - `Err(ConnectionClosed)` when the reader hits EOF (the typical
//!   "AirPods went away" exit),
//! - `Err(Protocol(_))` if the AirPods send something we know but
//!   can't parse (worth surfacing — bug in our codec),
//! - `Ok(())` if the receiving channel is dropped (UI shutting down).

use crate::TransportError;
use crate::event::DaemonEvent;
use crate::framing::read_frame;
use tokio::io::AsyncRead;

pub async fn run_read_loop<R: AsyncRead + Unpin>(
    reader: &mut R,
    events: &async_channel::Sender<DaemonEvent>,
) -> Result<(), TransportError> {
    loop {
        let frame = read_frame(reader).await?;
        if let Some(event) = DaemonEvent::from_frame(&frame)?
            && events.send(event).await.is_err()
        {
            // Receiver dropped — UI is shutting down. Clean exit.
            return Ok(());
        }
        // Unknown opcode (Ok(None)): silently skip.
    }
}

#[cfg(test)]
mod tests {
    use crate::TransportError;
    use crate::event::DaemonEvent;
    use crate::read_loop::run_read_loop;
    use tokio_test::io::Builder;

    /// Two pushed frames in a row (battery then ear-detection), then EOF.
    /// The loop should emit them in order to the channel and then return
    /// `ConnectionClosed`.
    #[tokio::test]
    async fn drains_frames_in_order_and_exits_on_eof() {
        const BATTERY: &[u8] = &[
            0x04, 0x00, 0x04, 0x00, 0x04, 0x00, // header
            0x01, 0x04, 0x01, 0x32, 0x02, 0x01, // 1 entry: Left, 50%, Discharging
        ];
        const EAR: &[u8] = &[0x04, 0x00, 0x04, 0x00, 0x06, 0x00, 0x00, 0x01];

        let mut mock = Builder::new().read(BATTERY).read(EAR).build();
        let (tx, rx) = async_channel::bounded(8);

        let task = tokio::spawn(async move { run_read_loop(&mut mock, &tx).await });

        match rx.recv().await.unwrap() {
            DaemonEvent::Battery(b) => assert_eq!(b.entries().len(), 1),
            other => panic!("expected Battery, got {other:?}"),
        }
        match rx.recv().await.unwrap() {
            DaemonEvent::EarDetection(_) => {}
            other => panic!("expected EarDetection, got {other:?}"),
        }

        let outcome = task.await.unwrap();
        assert!(matches!(outcome, Err(TransportError::ConnectionClosed)));
    }

    /// Receiver dropped before the reader hits EOF — loop should exit Ok(())
    /// rather than blocking forever or panicking.
    #[tokio::test]
    async fn exits_cleanly_when_receiver_dropped() {
        const BATTERY: &[u8] = &[
            0x04, 0x00, 0x04, 0x00, 0x04, 0x00, 0x01, 0x04, 0x01, 0x32, 0x02, 0x01,
        ];
        let mut mock = Builder::new().read(BATTERY).build();
        let (tx, rx) = async_channel::bounded::<DaemonEvent>(1);

        // Drop the receiver before the loop tries to send.
        drop(rx);

        let result = run_read_loop(&mut mock, &tx).await;
        assert!(matches!(result, Ok(())));
    }

    /// Unknown opcode in the middle is silently skipped; following frames
    /// still get through.
    #[tokio::test]
    async fn skips_unknown_opcodes() {
        const UNKNOWN: &[u8] = &[0x04, 0x00, 0x04, 0x00, 0xFE, 0x00, 0x11, 0x22];
        const EAR: &[u8] = &[0x04, 0x00, 0x04, 0x00, 0x06, 0x00, 0x00, 0x00];

        let mut mock = Builder::new().read(UNKNOWN).read(EAR).build();
        let (tx, rx) = async_channel::bounded(8);

        tokio::spawn(async move { run_read_loop(&mut mock, &tx).await });

        match rx.recv().await.unwrap() {
            DaemonEvent::EarDetection(_) => {}
            other => panic!("expected EarDetection (after skipped 0xFE), got {other:?}"),
        }
    }
}
