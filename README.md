# Game Linkd (`game-linkd`)

A generic, lightweight Linux daemon that exposes game detection and hardware inventory via a Varlink interface.

## Purpose

`game-linkd` allows userspace applications to:
1. **Detect active gaming sessions**: Seamlessly identify when a game starts or stops across multiple launchers.
2. **Hardware Inventory**: List connected gaming peripherals (keyboards, mice, controllers) and check their access permissions (`uaccess`).
3. **IPC Bridge**: Provides a root-level daemon that exposes a safe, unprivileged socket for user-level apps (like RGB controllers) to query system state.

## Features

- **Wide Support**: Detects games from:
  - **Steam**: Native and Flatpak versions.
  - **Heroic Games Launcher**: Epic Games, GOG, and Amazon Games.
  - **Lutris**: Open-source gaming platform for Linux.
  - **Process Detection**: (Coming soon) for generic standalone titles.
- **Hardware-Aware**: 
  - Groups complex udev nodes into single logical devices.
  - Identifies Manufacturer/Vendor names.
  - Reports `uaccess` status for "readiness" checks.
- **Modern IPC**: Uses [Varlink](https://varlink.org/) for typed, discoverable, and language-agnostic communication.
- **Zero-Dependency Core**: Distributed as a **systemd portable service**, bundling its own environment while remaining distro-agnostic.

## Installation (systemd portablectl)

The project is designed to be deployed using `portablectl`. This ensures all dependencies are bundled and it remains independent of the host OS.

### Build and Install
Use the provided automation script:

```bash
./scripts/install_portable.sh
```

This script will:
1. Build the Rust binary in release mode.
2. Assemble the portable OS tree in `./game-linkd/`.
3. Attach and start the service using `portablectl attach --now`.

### Interaction
You can use the provided script to query the daemon:

```bash
# Get active game (returns JSON if a game is running)
./scripts/gamelinkcli.sh active

# List installed games across Steam/Heroic
./scripts/gamelinkcli.sh list-games

# List connected peripherals
./scripts/gamelinkcli.sh list-devices
```

## Managing the Service

Once attached, treat it like any other systemd service:

```bash
sudo systemctl status game-linkd
sudo systemctl restart game-linkd
```

To detach/uninstall:
```bash
sudo portablectl detach game-linkd
```

## License

MIT / Apache 2.0
