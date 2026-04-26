// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors
// Portions derived from LibrePods (Copyright (C) 2025 LibrePods contributors).

//! Rename AirPods (AAP opcode `0x1A`).
//!
//! Spec format:
//!
//! ```text
//! 04 00 04 00 1A 00 01 [size] 00 [name_bytes]
//! ```
//!
//! Payload is `0x01 [size_byte] 0x00 [utf8_bytes]`. `size` is one
//! byte, so the longest name we can encode is 252 bytes — anything
//! longer is truncated to fit, on a UTF-8 character boundary so we
//! don't emit an invalid sequence.

/// AAP opcode for the rename command.
pub const OPCODE: u8 = 0x1A;

/// Largest name we can fit in the one-byte size field.
pub const MAX_NAME_LEN: usize = u8::MAX as usize;

/// Encode the rename payload (without the AAP frame header).
pub fn encode_rename(name: &str) -> Vec<u8> {
    let bytes = truncate_to_char_boundary(name, MAX_NAME_LEN).as_bytes();
    let mut payload = Vec::with_capacity(3 + bytes.len());
    payload.push(0x01);
    // bytes.len() <= MAX_NAME_LEN == u8::MAX as usize, so the cast is exact.
    payload.push(bytes.len() as u8);
    payload.push(0x00);
    payload.extend_from_slice(bytes);
    payload
}

fn truncate_to_char_boundary(name: &str, max_len: usize) -> &str {
    if name.len() <= max_len {
        return name;
    }
    let mut idx = max_len;
    while !name.is_char_boundary(idx) {
        idx -= 1;
    }
    &name[..idx]
}

#[cfg(test)]
mod tests {
    use crate::frame::Frame;
    use crate::rename::{MAX_NAME_LEN, OPCODE, encode_rename};

    /// Spec format: `04 00 04 00 1A 00 01 [size] 00 [name]`
    #[test]
    fn renames_to_pods_matches_spec() {
        let frame = Frame::encode(OPCODE, &encode_rename("Pods"));
        assert_eq!(
            frame,
            [
                0x04, 0x00, 0x04, 0x00, 0x1A, 0x00, // header
                0x01, 0x04, 0x00, // 0x01, size=4, 0x00
                b'P', b'o', b'd', b's',
            ],
        );
    }

    #[test]
    fn empty_name_yields_three_byte_payload() {
        let payload = encode_rename("");
        assert_eq!(payload, [0x01, 0x00, 0x00]);
    }

    #[test]
    fn multibyte_utf8_passes_through_unchanged() {
        // Unicode characters stay intact byte-for-byte.
        let payload = encode_rename("Поды");
        let bytes = "Поды".as_bytes();
        assert_eq!(payload[0], 0x01);
        assert_eq!(payload[1] as usize, bytes.len());
        assert_eq!(payload[2], 0x00);
        assert_eq!(&payload[3..], bytes);
    }

    #[test]
    fn over_long_name_truncated_at_char_boundary() {
        // (MAX_NAME_LEN - 1) ASCII bytes + one 2-byte char → MAX_NAME_LEN + 1
        // bytes total. The truncator must drop the multi-byte tail rather
        // than split it.
        let name = "a".repeat(MAX_NAME_LEN - 1) + "Я";
        let payload = encode_rename(&name);
        assert_eq!(payload[1] as usize, MAX_NAME_LEN - 1);
        assert_eq!(&payload[3..], "a".repeat(MAX_NAME_LEN - 1).as_bytes());
    }
}
