// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Read one AAP frame from any `AsyncRead`.
//!
//! The L2CAP socket we get from `bluer` is `SeqPacket`-flavoured: every
//! `recv` returns exactly one L2CAP packet, and every L2CAP packet from
//! the AirPods carries exactly one AAP frame. So "read a frame" reduces
//! to "read once and parse the bytes." If we ever switch to a stream
//! where frames can split across reads, this is the place to add a
//! length-delimited buffer.

use crate::TransportError;
use tokio::io::{AsyncRead, AsyncReadExt};
use tragus_protocol::{Frame, OwnedFrame};

/// Generous buffer for a single L2CAP packet. AirPods packets in the
/// wild peak around 140 bytes (EQ_DATA); 1024 leaves ample headroom.
const READ_BUFFER_SIZE: usize = 1024;

pub async fn read_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<OwnedFrame, TransportError> {
    let mut buf = vec![0u8; READ_BUFFER_SIZE];
    let n = reader.read(&mut buf).await?;
    if n == 0 {
        return Err(TransportError::ConnectionClosed);
    }
    let frame = Frame::parse(&buf[..n])?;
    Ok(frame.into())
}

#[cfg(test)]
mod tests {
    use crate::TransportError;
    use crate::framing::read_frame;
    use tokio_test::io::Builder;

    /// Reference battery packet from `AAP Definitions.md`. The mock reader
    /// hands the entire packet to a single `read` call, mirroring how
    /// `bluer::l2cap::Stream` (SeqPacket) delivers exactly one packet
    /// per recv.
    #[tokio::test]
    async fn reads_battery_packet_into_owned_frame() {
        const REFERENCE: &[u8] = &[
            0x04, 0x00, 0x04, 0x00, 0x04, 0x00, // header (opcode 0x04)
            0x03, // 3 entries
            0x02, 0x01, 0x64, 0x02, 0x01, // Right, 100%, Discharging
            0x04, 0x01, 0x63, 0x01, 0x01, // Left,  99%, Charging
            0x08, 0x01, 0x11, 0x02, 0x01, // Case,  17%, Discharging
        ];

        let mut mock = Builder::new().read(REFERENCE).build();

        let frame = read_frame(&mut mock).await.unwrap();
        assert_eq!(frame.opcode, 0x04);
        assert_eq!(frame.payload.len(), 16);
        assert_eq!(frame.payload[0], 0x03);
    }

    #[tokio::test]
    async fn empty_read_means_connection_closed() {
        let mut mock = Builder::new().build();
        match read_frame(&mut mock).await {
            Err(TransportError::ConnectionClosed) => {}
            other => panic!("expected ConnectionClosed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn malformed_frame_surfaces_protocol_error() {
        // Wrong prefix (0x05 instead of 0x04) — should bubble out of the
        // protocol layer through TransportError::Protocol.
        let mut mock = Builder::new()
            .read(&[0x05, 0x00, 0x04, 0x00, 0x04, 0x00])
            .build();
        match read_frame(&mut mock).await {
            Err(TransportError::Protocol(_)) => {}
            other => panic!("expected Protocol error, got {other:?}"),
        }
    }
}
