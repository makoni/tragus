// SPDX-License-Identifier: GPL-3.0-or-later
//
// Tragus — native GNOME app for AirPods on Linux.
// Copyright (C) 2026 Tragus contributors

//! Pure-Rust crypto helpers for the Bluetooth side of the AirPods
//! protocol.
//!
//! - [`aes128_ecb_encrypt`] — single-block AES-128 ECB. Used to
//!   decrypt the encrypted tail of Apple's BLE proximity-pairing
//!   advertisement (PSM 0x004C, last 16 bytes, key = ENC_KEY from
//!   AAP opcode 0x30/0x31).
//! - [`ah`] — Bluetooth resolvable-private-address hash. Lets us
//!   verify that a private address belongs to a paired AirPods
//!   given its IRK, when BlueZ doesn't already give us the
//!   identity address.
//!
//! Pulls in `aes` (RustCrypto) — pure Rust, no FFI, audited; we never
//! need anything stronger than ECB single-block here.

use aes::cipher::{BlockCipherEncrypt, KeyInit};
use aes::{Aes128, Block};

/// Encrypt one 16-byte block in place with AES-128 ECB.
pub fn aes128_ecb_encrypt(key: &[u8; 16], block: &mut [u8; 16]) {
    let cipher = Aes128::new(&Block::from(*key));
    let mut buf = Block::from(*block);
    cipher.encrypt_block(&mut buf);
    *block = buf.into();
}

/// Bluetooth Core Spec v5.4 Vol 3 Part H §2.2.2 — random-address hash.
///
/// `ah(IRK, prand) = AES-128(IRK, prand_padded_to_16_bytes)[..3]`
///
/// The padding fills the most-significant 13 bytes with zero, leaving
/// the 3-byte `prand` in the least-significant position. Returns the
/// least-significant 3 bytes of the cipher output.
pub fn ah(irk: &[u8; 16], prand: &[u8; 3]) -> [u8; 3] {
    let mut block = [0u8; 16];
    block[13] = prand[0];
    block[14] = prand[1];
    block[15] = prand[2];
    aes128_ecb_encrypt(irk, &mut block);
    [block[13], block[14], block[15]]
}

#[cfg(test)]
mod tests {
    use crate::crypto::{aes128_ecb_encrypt, ah};

    /// BT Core Spec v5.4 Vol 3 Part H, sample data for `ah()`:
    /// IRK = `0xec0234a357c8ad05341010a60a397d9b`
    /// r   = `0x708194`  (3 bytes, "prand")
    /// expected hash = `0x0dfbaa`
    #[test]
    fn ah_matches_bt_core_spec_test_vector() {
        let irk: [u8; 16] = [
            0xec, 0x02, 0x34, 0xa3, 0x57, 0xc8, 0xad, 0x05, 0x34, 0x10, 0x10, 0xa6, 0x0a, 0x39,
            0x7d, 0x9b,
        ];
        let prand: [u8; 3] = [0x70, 0x81, 0x94];
        let hash = ah(&irk, &prand);
        assert_eq!(hash, [0x0d, 0xfb, 0xaa]);
    }

    /// AES-128 ECB known-answer (FIPS-197 Appendix B):
    /// key       = 2b7e151628aed2a6abf7158809cf4f3c
    /// plaintext = 6bc1bee22e409f96e93d7e117393172a
    /// expected  = 3ad77bb40d7a3660a89ecaf32466ef97
    #[test]
    fn aes128_ecb_matches_fips_197_test_vector() {
        let key: [u8; 16] = [
            0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf,
            0x4f, 0x3c,
        ];
        let mut block: [u8; 16] = [
            0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93,
            0x17, 0x2a,
        ];
        aes128_ecb_encrypt(&key, &mut block);
        assert_eq!(
            block,
            [
                0x3a, 0xd7, 0x7b, 0xb4, 0x0d, 0x7a, 0x36, 0x60, 0xa8, 0x9e, 0xca, 0xf3, 0x24, 0x66,
                0xef, 0x97,
            ],
        );
    }
}
