# Context Daemon (`contextd`)

A generic, lightweight Linux daemon that exposes process context (e.g., gaming activity) and hardware inventory via a Varlink interface.

## Purpose

`contextd` allows userspace applications to:
1. **Detect active sessions**: Seamlessly identify when a high-performance app or game starts or stops across multiple launchers.
2. **Hardware Inventory**: List connected peripherals (keyboards, mice, controllers) and check their access permissions (`uaccess`).
3. **IPC Bridge**: Provides a root-level daemon that exposes a safe, unprivileged socket for user-level apps (like RGB controllers or profile switchers) to query system state.

## Features

- **Wide Support**: Detects games and apps from:
  - **Steam**: Native and Flatpak versions.
  - **Heroic Games Launcher**: Epic Games, GOG, and Amazon Games.
  - **Lutris**: Open-source gaming platform for Linux.
- **Hardware-Aware**: 
  - Groups complex udev nodes into single logical devices.
  - **Main Inventory**: Clean list of only active gaming gear (Mice, Keyboards, Controllers, Audio).
  - **RGB Inventory**: Dedicated endpoint for system aesthetics (LEDs, Fans, Lighting Strips).
  - Reports `uaccess` status for "readiness" checks (permission verification).
- **Modern IPC**: Uses [Varlink](https://varlink.org/) for typed, discoverable, and language-agnostic communication.
- **systemd Native**: Distributed as a **systemd portable service**, ensuring zero-dependency deployment on any modern Linux distro.

## Installation (systemd portablectl)

### Build and Install
Use the provided automation script:

```bash
./scripts/install_portable.sh
```

This script builds the Rust binary, assembles the portable OS tree in `./contextd/`, and attaches the service using `portablectl`.

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
```

## Debugging & Testing

A Python-based dashboard is available to inspect the daemon state:

```bash
# Get a pretty-printed system dashboard
./tests/debug_tools.py dash
```

## Managing the Service

```bash
sudo systemctl status contextd
sudo systemctl restart contextd
```

To detach/uninstall:
```bash
sudo portablectl detach contextd
```

## License

MIT / Apache 2.0
