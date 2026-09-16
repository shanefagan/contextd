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
3. **IPC Ambient Lighting Bridge**: Exposes unprivileged sockets for user-level apps and hardware micro-daemons (like `argbd` or OpenRGB bridges) to publish and consume ambient lighting vibe contexts.

## Workspace Architecture

```
contextd/
├── src/                            # contextd Core System & RGB Daemon
├── contextd-client/                # Official Rust Client Library (contextd-client)
└── bridges/
    └── openrgb-contextd-bridge/    # Optional OpenRGB SDK TCP Bridge (Port 6742)
```

### Components

- **`contextd`**: Core daemon serving game detection, hardware inventory, and RGB observer/control Varlink sockets.
- **`contextd-client`**: Official Rust client crate providing type-safe Rust interfaces (`ObserverClient`, `ControlClient`) with automatic connection recovery.
- **`openrgb-contextd-bridge`**: Optional TCP server bridge on port 6742 that accepts OpenRGB SDK RGB updates and forwards them to `contextd`.

## Features

- **Wide Launcher Support**: Detects games and apps from Steam (Native/Flatpak), Heroic Games Launcher, and Lutris.
- **Hardware-Aware**: Groups complex udev nodes into single logical devices; provides main hardware and dedicated RGB inventory.
- **System Diagnostics**: Reports RAM, CPU, GPU specs, kernel version, and verifies Vulkan and OpenGL libraries.
- **Modern IPC**: Uses [Varlink](https://varlink.org/) over Unix sockets (`/run/contextd/public/contextd-rgb-observer.socket` and `/run/contextd/private/contextd-rgb-control.socket`).
- **systemd Native**: Distributed as a **systemd portable service** (`portablectl`) for zero-dependency Linux deployment.

## Installation & Deployment (systemd portablectl)

### Universal Deployment
Run the production deployment script to assemble the portable OS tree in `/opt/contextd` and manage `portablectl` attachment:
```bash
./scripts/deploy.sh
```

### Optional OpenRGB SDK Bridge Service
To start the optional OpenRGB SDK bridge daemon on TCP port 6742:
```bash
sudo systemctl enable --now openrgb-contextd-bridge.service
```

### Interaction
Use the `contextctl` helper to query the daemon:
```bash
# Get active game/app
./scripts/contextctl.sh active

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

- **Unprivileged Observer Socket**: Ambient lighting vibe stream is public via `/run/contextd/public/contextd-rgb-observer.socket`.
- **Restricted Control Socket**: Writing lighting state changes is handled via `/run/contextd/private/contextd-rgb-control.socket`.
- **Peer Validation**: Uses `SO_PEERCRED` to identify systemd units for restricted operations.

## Development & Verification

Build and run CI tests across the workspace:
```bash
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
```

## License

MIT
