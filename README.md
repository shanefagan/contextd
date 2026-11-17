<p align="center">
  <img src="assets/logo.png" width="200" alt="contextd logo">
</p>

# Context Daemon (`contextd`)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE.md)

A generic, lightweight Linux daemon that exposes process context (e.g., gaming activity), hardware inventory, and system ambient lighting context via Varlink interfaces.

## Purpose

`contextd` allows userspace applications to:
1. **Detect active sessions**: Seamlessly identify when a high-performance app or game starts or stops across multiple launchers.
2. **Hardware Inventory**: List connected peripherals (keyboards, mice, controllers) and check their access permissions (`uaccess`).
3. **IPC Ambient Lighting Bridge**: Lets user-level apps and hardware micro-daemons (like `argbd` or OpenRGB bridges) publish and consume an ambient lighting "vibe". Anyone can read it; writing is limited to the `contextd-rgb` group.

## Workspace Architecture

```
contextd/
├── src/                            # contextd Core System & RGB Daemon
├── contextd-client/                # Official Rust Client Library (contextd-client)
│   └── interfaces/                 # Varlink interface definitions (source of truth)
└── bridges/
    └── openrgb-contextd-bridge/    # Optional OpenRGB SDK TCP Bridge (Port 6742)
```

### Components

- **`contextd`**: Core daemon serving game detection, hardware inventory, and RGB observer/control Varlink sockets.
- **`contextd-client`**: Official Rust client crate providing type-safe interfaces (`CoreClient`, `ObserverClient`, `ControlClient`) with automatic reconnection. The daemon is built on the same generated types.
- **`openrgb-contextd-bridge`**: Optional TCP server bridge on port 6742 that accepts OpenRGB SDK RGB updates and forwards them to `contextd`.

## Features

- **Wide Launcher Support**: Detects installed and running games from Steam (Native/Flatpak), Heroic Games Launcher (Epic, GOG, Amazon), and Lutris.
- **Hardware-Aware**: Groups udev nodes by physical device (so two identical controllers stay two devices) and lists every node of each device.
- **System Diagnostics**: Reports RAM, CPU, GPU specs, kernel version, and verifies Vulkan and OpenGL libraries.
- **Modern IPC**: Uses [Varlink](https://varlink.org/) over Unix sockets (`/run/contextd/public/contextd.socket`, `/run/contextd/public/contextd-rgb-observer.socket` and `/run/contextd/private/contextd-rgb-control.socket`).
- **systemd Native**: Distributed as a **systemd portable service** (`portablectl`) with socket activation: the daemons start when a client first connects.

## Installation & Deployment (systemd portablectl)

### Universal Deployment
Run the production deployment script to assemble the portable OS tree in `/opt/contextd`, create the `contextd-rgb` group, install the OpenRGB bridge, and attach and enable the service with `portablectl`:
```bash
./scripts/deploy.sh
```
`portablectl` comes from the `systemd-container` package on Debian, Ubuntu and Fedora, and with `systemd` on Arch.

To let a user change lighting:
```bash
sudo usermod -aG contextd-rgb "$USER"   # log in again afterwards
```

### Optional OpenRGB SDK Bridge Service
To start the optional OpenRGB SDK bridge daemon on TCP port 6742 (127.0.0.1 only):
```bash
sudo systemctl enable --now openrgb-contextd-bridge.service
```
OpenRGB's own SDK server uses the same port; set `OPENRGB_SERVER_PORT` in the unit if you run both.

### Interaction
Use the `contextctl` helper to query the daemon:
```bash
# Get the most recently launched running game
./scripts/contextctl.sh active

# List all running games
./scripts/contextctl.sh running

# List installed games
./scripts/contextctl.sh list-games

# List connected gaming peripherals
./scripts/contextctl.sh list-devices

# List RGB controllers, fans, and lights
./scripts/contextctl.sh list-rgb

# Get system diagnostics
./scripts/contextctl.sh diagnostics
```

## Security & Access Control

- **Public sockets**: The core socket and the lighting observer socket are readable by everyone.
- **Restricted control socket**: `/run/contextd/private/contextd-rgb-control.socket` is mode 0660, group `contextd-rgb`. `auth.authorized_units` can narrow it to specific system services.
- **Peer validation**: `SO_PEERCRED` (and `SO_PEERPIDFD` where available) identifies callers: controller hints are bound to the caller's own processes, and only system-slice units can match the whitelist.
- **Limits**: Connections per user, message sizes and idle time are capped, so one local user cannot starve the daemon.
- **Fail closed**: A config file that exists but does not parse stops the daemon instead of falling back to defaults.

See [docs/security.md](docs/security.md) for details.

## Development & Verification

Build and run CI tests across the workspace:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run a development instance without root or systemd; sockets go under the runtime directory:
```bash
cargo run -- --runtime-dir /tmp/contextd --config examples/config.sample.toml
cargo run -- --rgb --runtime-dir /tmp/contextd
```

## License

MIT
