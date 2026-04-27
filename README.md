# Tragus

A native GNOME application for managing AirPods on Linux. Built with Rust, GTK 4, and libadwaita.

> **Tragus** *(noun)* — the small pointed eminence of the external ear in front of the concha.

## Status

Early development. Nothing connects to your AirPods yet — this is a fresh
project skeleton. Expect breaking changes.

## Relationship to LibrePods

Tragus is a from-scratch Rust + GTK port of the AirPods integration features
researched and pioneered by [LibrePods](https://github.com/kavishdevar/librepods).
Protocol logic is being ported in (with full attribution) from the LibrePods
Android app, which is the most complete open-source AirPods client today.

LibrePods itself ships a Qt-based Linux client; Tragus exists because GNOME
users deserve a native libadwaita client with feature parity to the Android
app, and because the Qt client's hearing-aid features live in a separate
Python script due to a QtBluetooth limitation that does not affect a
BlueZ-native Rust implementation.

## Project layout

```
tragus/
├── crates/
│   ├── tragus-protocol/    Pure-Rust AAP packet codec. No I/O, no GTK.
│   ├── tragus-bluetooth/   BlueZ + L2CAP transport via `bluer`.
│   └── tragus/             GTK 4 + libadwaita UI (main binary).
└── data/                   .desktop, AppStream metainfo, icons (TBD).
```

## Building

System dependencies (Fedora):

```bash
sudo dnf install gtk4-devel libadwaita-devel bluez-libs-devel openssl-devel pkgconf
```

System dependencies (Debian/Ubuntu):

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev libbluetooth-dev libssl-dev pkg-config
```

System dependencies (Arch):

```bash
sudo pacman -S gtk4 libadwaita bluez-libs openssl pkgconf
```

Then:

```bash
cargo run -p tragus
```

Requires Rust 1.95+ (edition 2024).

## Running

```bash
# Real Bluetooth: needs paired AirPods and BlueZ ≥ 5.56.
cargo run -p tragus

# UI demo without hardware (cycles ANC modes, fake battery levels):
cargo run -p tragus -- --fake
```

## Debugging

Tracing is on by default at `debug` level for the project's three crates,
so a normal run already shows every parsed packet, every command sent,
and every event emitted:

```text
INFO  tragus_bluetooth::discovery: scanning paired BlueZ devices count=3
INFO  tragus_bluetooth::discovery: found AirPods candidate addr=AA:BB:… name=Some("AirPods Pro")
INFO  tragus_bluetooth::discovery: opening AAP L2CAP socket addr=… psm=0x1001
INFO  tragus_bluetooth::handshake: sending AAP init sequence
DEBUG tragus_bluetooth::framing: ← AAP frame opcode=0x04 payload_len=16
DEBUG tragus_bluetooth::read_loop: ↑ DaemonEvent event=Battery(...)
DEBUG tragus::bridge: bridge: applying event to AirPodsState
DEBUG tragus_bluetooth::command_loop: ↓ DaemonCommand cmd=SetListeningMode(NoiseCancellation)
```

For raw byte dumps, set `RUST_LOG` explicitly:

```bash
RUST_LOG=tragus=trace,tragus_bluetooth=trace cargo run -p tragus
```

To go quiet:

```bash
RUST_LOG=warn cargo run -p tragus
```

When reporting issues, paste the `info` + `debug` block from app start
through the moment something went wrong; the `trace` raw bytes are only
useful when the protocol parser itself is the suspect.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).

This project incorporates work derived from
[LibrePods](https://github.com/kavishdevar/librepods)
(Copyright © 2025 LibrePods contributors), which is licensed under
GPL-3.0-or-later. Each ported file retains the original copyright notice
alongside the Tragus copyright notice, as required by GPL §5.

## Acknowledgements

- The [LibrePods](https://github.com/kavishdevar/librepods) project for the
  protocol research, reference implementation, and the Android app this
  port draws from.
- [@tyalie](https://github.com/tyalie) for the original
  [AAP protocol documentation](https://github.com/tyalie/AAP-Protocol-Defintion).
- [@rithvikvibhu](https://github.com/rithvikvibhu) and lagrangepoint for the
  hearing-aid protocol work.

## Trademarks

AirPods is a trademark of Apple Inc. This project is not affiliated with,
endorsed by, or sponsored by Apple Inc.
