// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! ATT (Attribute Protocol) PDU codec.
//!
//! This is the byte-level encoder/decoder for the slice of ATT we need
//! to talk to AirPods' GATT characteristics — specifically:
//!
//! - 0x18 TRANSPARENCY (100-byte payload)
//! - 0x1B LOUD_SOUND_REDUCTION
//! - 0x2A HEARING_AID  (104-byte payload)
//!
//! ATT runs over its own L2CAP socket at PSM 0x1F. The transport
//! layer (`tragus-bluetooth`) opens it; this module just translates
//! `AttPdu` values to/from bytes.
//!
//! Reference: Bluetooth Core Spec v5.4 Vol 3 Part F.

use crate::error::ProtocolError;

pub const READ_REQUEST_OPCODE: u8 = 0x0A;
pub const READ_RESPONSE_OPCODE: u8 = 0x0B;
pub const WRITE_REQUEST_OPCODE: u8 = 0x12;
pub const WRITE_RESPONSE_OPCODE: u8 = 0x13;
pub const NOTIFICATION_OPCODE: u8 = 0x1B;
pub const ERROR_RESPONSE_OPCODE: u8 = 0x01;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttPdu {
    /// `0A | handle(u16 LE)`
    ReadRequest { handle: u16 },
    /// `0B | value(...)`
    ReadResponse { value: Vec<u8> },
    /// `12 | handle(u16 LE) | value(...)`
    WriteRequest { handle: u16, value: Vec<u8> },
    /// `13` (no payload).
    WriteResponse,
    /// `1B | handle(u16 LE) | value(...)`
    Notification { handle: u16, value: Vec<u8> },
    /// `01 | request_opcode | handle(u16 LE) | error_code`
    ErrorResponse {
        request_opcode: u8,
        handle: u16,
        error: ErrorCode,
    },
}

impl AttPdu {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::ReadRequest { handle } => {
                let mut buf = Vec::with_capacity(3);
                buf.push(READ_REQUEST_OPCODE);
                buf.extend_from_slice(&handle.to_le_bytes());
                buf
            }
            Self::ReadResponse { value } => {
                let mut buf = Vec::with_capacity(1 + value.len());
                buf.push(READ_RESPONSE_OPCODE);
                buf.extend_from_slice(value);
                buf
            }
            Self::WriteRequest { handle, value } => {
                let mut buf = Vec::with_capacity(3 + value.len());
                buf.push(WRITE_REQUEST_OPCODE);
                buf.extend_from_slice(&handle.to_le_bytes());
                buf.extend_from_slice(value);
                buf
            }
            Self::WriteResponse => vec![WRITE_RESPONSE_OPCODE],
            Self::Notification { handle, value } => {
                let mut buf = Vec::with_capacity(3 + value.len());
                buf.push(NOTIFICATION_OPCODE);
                buf.extend_from_slice(&handle.to_le_bytes());
                buf.extend_from_slice(value);
                buf
            }
            Self::ErrorResponse {
                request_opcode,
                handle,
                error,
            } => {
                let mut buf = Vec::with_capacity(5);
                buf.push(ERROR_RESPONSE_OPCODE);
                buf.push(*request_opcode);
                buf.extend_from_slice(&handle.to_le_bytes());
                buf.push(u8::from(*error));
                buf
            }
        }
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let Some(&opcode) = bytes.first() else {
            return Err(ProtocolError::TooShort {
                expected: 1,
                got: 0,
            });
        };
        match opcode {
            READ_REQUEST_OPCODE => {
                let handle = parse_handle(&bytes[1..])?;
                Ok(Self::ReadRequest { handle })
            }
            READ_RESPONSE_OPCODE => Ok(Self::ReadResponse {
                value: bytes[1..].to_vec(),
            }),
            WRITE_REQUEST_OPCODE => {
                let handle = parse_handle(&bytes[1..])?;
                Ok(Self::WriteRequest {
                    handle,
                    value: bytes[3..].to_vec(),
                })
            }
            WRITE_RESPONSE_OPCODE => Ok(Self::WriteResponse),
            NOTIFICATION_OPCODE => {
                let handle = parse_handle(&bytes[1..])?;
                Ok(Self::Notification {
                    handle,
                    value: bytes[3..].to_vec(),
                })
            }
            ERROR_RESPONSE_OPCODE => {
                if bytes.len() < 5 {
                    return Err(ProtocolError::TooShort {
                        expected: 5,
                        got: bytes.len(),
                    });
                }
                let handle = parse_handle(&bytes[2..])?;
                Ok(Self::ErrorResponse {
                    request_opcode: bytes[1],
                    handle,
                    error: ErrorCode::from_byte(bytes[4]),
                })
            }
            other => Err(ProtocolError::UnknownOpcode(other)),
        }
    }
}

fn parse_handle(rest: &[u8]) -> Result<u16, ProtocolError> {
    if rest.len() < 2 {
        return Err(ProtocolError::TooShort {
            expected: 3,
            got: 1 + rest.len(),
        });
    }
    Ok(u16::from_le_bytes([rest[0], rest[1]]))
}

/// ATT error codes — the subset we care about; everything else lands
/// in `Other(byte)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidHandle,
    ReadNotPermitted,
    WriteNotPermitted,
    InvalidPdu,
    InsufficientAuthentication,
    RequestNotSupported,
    InvalidOffset,
    InsufficientAuthorization,
    PrepareQueueFull,
    AttributeNotFound,
    AttributeNotLong,
    InsufficientEncryptionKeySize,
    InvalidAttributeValueLength,
    UnlikelyError,
    InsufficientEncryption,
    UnsupportedGroupType,
    InsufficientResources,
    Other(u8),
}

impl ErrorCode {
    fn from_byte(b: u8) -> Self {
        match b {
            0x01 => Self::InvalidHandle,
            0x02 => Self::ReadNotPermitted,
            0x03 => Self::WriteNotPermitted,
            0x04 => Self::InvalidPdu,
            0x05 => Self::InsufficientAuthentication,
            0x06 => Self::RequestNotSupported,
            0x07 => Self::InvalidOffset,
            0x08 => Self::InsufficientAuthorization,
            0x09 => Self::PrepareQueueFull,
            0x0A => Self::AttributeNotFound,
            0x0B => Self::AttributeNotLong,
            0x0C => Self::InsufficientEncryptionKeySize,
            0x0D => Self::InvalidAttributeValueLength,
            0x0E => Self::UnlikelyError,
            0x0F => Self::InsufficientEncryption,
            0x10 => Self::UnsupportedGroupType,
            0x11 => Self::InsufficientResources,
            other => Self::Other(other),
        }
    }
}

impl From<ErrorCode> for u8 {
    fn from(c: ErrorCode) -> u8 {
        match c {
            ErrorCode::InvalidHandle => 0x01,
            ErrorCode::ReadNotPermitted => 0x02,
            ErrorCode::WriteNotPermitted => 0x03,
            ErrorCode::InvalidPdu => 0x04,
            ErrorCode::InsufficientAuthentication => 0x05,
            ErrorCode::RequestNotSupported => 0x06,
            ErrorCode::InvalidOffset => 0x07,
            ErrorCode::InsufficientAuthorization => 0x08,
            ErrorCode::PrepareQueueFull => 0x09,
            ErrorCode::AttributeNotFound => 0x0A,
            ErrorCode::AttributeNotLong => 0x0B,
            ErrorCode::InsufficientEncryptionKeySize => 0x0C,
            ErrorCode::InvalidAttributeValueLength => 0x0D,
            ErrorCode::UnlikelyError => 0x0E,
            ErrorCode::InsufficientEncryption => 0x0F,
            ErrorCode::UnsupportedGroupType => 0x10,
            ErrorCode::InsufficientResources => 0x11,
            ErrorCode::Other(b) => b,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::att::{AttPdu, ErrorCode, READ_REQUEST_OPCODE, WRITE_REQUEST_OPCODE};
    use crate::error::ProtocolError;

    /// Read Request: `0A | handle (u16 LE)`
    #[test]
    fn read_request_round_trip() {
        let req = AttPdu::ReadRequest { handle: 0x0018 };
        let bytes = req.encode();
        assert_eq!(bytes, [READ_REQUEST_OPCODE, 0x18, 0x00]);
        assert_eq!(AttPdu::parse(&bytes).unwrap(), req);
    }

    /// Read Response: `0B | value`
    #[test]
    fn read_response_round_trip() {
        let resp = AttPdu::ReadResponse {
            value: vec![0x42, 0xDE, 0xAD],
        };
        let bytes = resp.encode();
        assert_eq!(bytes, [0x0B, 0x42, 0xDE, 0xAD]);
        assert_eq!(AttPdu::parse(&bytes).unwrap(), resp);
    }

    /// Write Request: `12 | handle (u16 LE) | value`
    #[test]
    fn write_request_round_trip() {
        let req = AttPdu::WriteRequest {
            handle: 0x002A,
            value: vec![0x01, 0x02, 0x03],
        };
        let bytes = req.encode();
        assert_eq!(bytes, [WRITE_REQUEST_OPCODE, 0x2A, 0x00, 0x01, 0x02, 0x03]);
        assert_eq!(AttPdu::parse(&bytes).unwrap(), req);
    }

    /// Write Response: opcode only (`13`).
    #[test]
    fn write_response_round_trip() {
        let resp = AttPdu::WriteResponse;
        assert_eq!(resp.encode(), [0x13]);
        assert_eq!(AttPdu::parse(&[0x13]).unwrap(), resp);
    }

    /// Handle Value Notification: `1B | handle | value`
    #[test]
    fn notification_round_trip() {
        let n = AttPdu::Notification {
            handle: 0x0018,
            value: vec![0xAA, 0xBB],
        };
        let bytes = n.encode();
        assert_eq!(bytes, [0x1B, 0x18, 0x00, 0xAA, 0xBB]);
        assert_eq!(AttPdu::parse(&bytes).unwrap(), n);
    }

    /// Error Response: `01 | request_opcode | handle (u16 LE) | error_code`
    #[test]
    fn error_response_round_trip() {
        let e = AttPdu::ErrorResponse {
            request_opcode: WRITE_REQUEST_OPCODE,
            handle: 0x002A,
            error: ErrorCode::AttributeNotFound,
        };
        let bytes = e.encode();
        assert_eq!(bytes, [0x01, 0x12, 0x2A, 0x00, 0x0A]);
        assert_eq!(AttPdu::parse(&bytes).unwrap(), e);
    }

    #[test]
    fn unknown_opcode_returns_error() {
        let bytes = [0xFE, 0x00, 0x00];
        assert!(matches!(
            AttPdu::parse(&bytes),
            Err(ProtocolError::UnknownOpcode(0xFE)),
        ));
    }

    #[test]
    fn truncated_read_request_too_short() {
        // 0x0A but no handle bytes.
        assert!(matches!(
            AttPdu::parse(&[READ_REQUEST_OPCODE]),
            Err(ProtocolError::TooShort { .. }),
        ));
    }
}
