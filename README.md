# Game Linkd (`game-linkd`)

A generic, lightweight Linux daemon that exposes game detection and hardware inventory via a Varlink interface.

## Purpose

`game-linkd` allows userspace applications to:
1. Detect if a game is currently running.
2. List connected gaming peripherals.

## Design Goals

- **Generic**: Works with Steam, Lutris, and native binaries.
- **Modern**: Uses [Varlink](https://varlink.org/) for IPC.
- **Portable**: Distributed as a systemd portable service.

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
3. Attach the image using `portablectl attach --now`.

#### Testing with the CLI
You can use the `gamelinkcli` tool to interact with the daemon:

```bash
# Get active game
./scripts/gamelinkcli.sh active

# List installed games
./scripts/gamelinkcli.sh list-games
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
