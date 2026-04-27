// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! ATT transport over the dedicated L2CAP socket at PSM 0x1F.
//!
//! AirPods expose three GATT characteristics that drive the
//! transparency / hearing-aid / loud-sound-reduction screens. They
//! live behind a separate L2CAP-PSM socket from AAP. This module is
//! the read/write half — `tragus_protocol::att` handles the byte
//! layout, we just push PDUs through the socket and surface PDUs that
//! arrive as notifications.

use crate::TransportError;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tragus_protocol::att::AttPdu;

const READ_BUFFER: usize = 256;

pub async fn read_att_pdu<R: AsyncRead + Unpin>(reader: &mut R) -> Result<AttPdu, TransportError> {
    let mut buf = vec![0u8; READ_BUFFER];
    let n = reader.read(&mut buf).await?;
    if n == 0 {
        tracing::debug!("ATT socket EOF (read returned 0 bytes)");
        return Err(TransportError::ConnectionClosed);
    }
    tracing::trace!(len = n, raw = ?&buf[..n], "← ATT raw");
    let pdu = AttPdu::parse(&buf[..n])?;
    tracing::debug!(?pdu, "← ATT PDU");
    Ok(pdu)
}

pub async fn write_att_pdu<W: AsyncWrite + Unpin>(
    writer: &mut W,
    pdu: &AttPdu,
) -> Result<(), TransportError> {
    tracing::debug!(?pdu, "→ ATT PDU");
    let bytes = pdu.encode();
    tracing::trace!(?bytes, "→ ATT raw");
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::TransportError;
    use crate::att_session::{read_att_pdu, write_att_pdu};
    use tokio_test::io::Builder;
    use tragus_protocol::att::AttPdu;

    /// `0A | 18 00` — Read Request for handle 0x0018.
    #[tokio::test]
    async fn write_read_request_emits_three_bytes() {
        let mut mock = Builder::new().write(&[0x0A, 0x18, 0x00]).build();
        write_att_pdu(&mut mock, &AttPdu::ReadRequest { handle: 0x0018 })
            .await
            .unwrap();
    }

    /// Notification for handle 0x0018 with a four-byte payload.
    #[tokio::test]
    async fn read_notification_returns_typed_pdu() {
        let mut mock = Builder::new()
            .read(&[0x1B, 0x18, 0x00, 0x01, 0x02, 0x03, 0x04])
            .build();
        let pdu = read_att_pdu(&mut mock).await.unwrap();
        assert_eq!(
            pdu,
            AttPdu::Notification {
                handle: 0x0018,
                value: vec![0x01, 0x02, 0x03, 0x04],
            },
        );
    }

    #[tokio::test]
    async fn empty_read_means_connection_closed() {
        let mut mock = Builder::new().build();
        match read_att_pdu(&mut mock).await {
            Err(TransportError::ConnectionClosed) => {}
            other => panic!("expected ConnectionClosed, got {other:?}"),
        }
    }
}
