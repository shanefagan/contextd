# Contextd Examples

This directory contains examples of how to interact with the `contextd` daemon from different programming languages.

Because `contextd` uses [Varlink](https://varlink.org) over standard Unix sockets, you can use any language that supports Unix domain sockets and JSON serialization.

## Available Examples

### 1. Python
Python is a great choice for scripting against `contextd` because it has built-in support for Unix sockets and JSON, requiring zero external dependencies.
- **`python/get_active_game.py`**: Queries the core daemon for the most recently launched running game.
- **`python/subscribe_rgb.py`**: Subscribes to real-time ambient lighting updates from the RGB observer socket.
- **`python/register_controller.py`**: Demonstrates how to register a hardware controller "hint" to coordinate with other apps.

### 2. Shell
For simple queries in bash scripts, `varlinkctl` (part of systemd 255 and later) allows one-line interactions.
- **`shell/get_diagnostics.sh`**: Uses `varlinkctl` to query system diagnostics.
- **`shell/register_controller.sh`**: Registers a temporary hint using a bash script.

### 3. Rust
The easiest route is the `contextd-client` crate in this repository (`CoreClient`, `ObserverClient`, `ControlClient`). These examples show that you can also do it with nothing but the standard library and `serde_json`.
- **`rust/src/main.rs`**: A standard Cargo project showing how to connect to the Unix socket and parse a JSON response.
- **`rust/src/bin/register_controller.rs`**: Shows how to register a controller hint using raw Unix sockets and Serde.
  *Run it with:* `cd rust && cargo run --bin register_controller`

### 4. C
A minimal C example demonstrating how to use POSIX sockets to send a Varlink JSON request and read the response.
- **`c/get_active_game.c`**: Pure C with no external dependencies (uses standard `sys/socket.h` and `sys/un.h`).
  *Compile and run:* `gcc c/get_active_game.c -o get_active_game && ./get_active_game`

## Flatpak Integration
If you are developing a sandboxed application (like a Flatpak), you need to grant it access to the daemon's sockets.
- See [FLATPAK.md](FLATPAK.md) for manifest configuration and command-line overrides.

## Socket Paths
Remember the daemon exposes three interfaces:
1. **Core Socket**: `/run/contextd/public/contextd.socket`
2. **RGB Observer**: `/run/contextd/public/contextd-rgb-observer.socket`
3. **RGB Control**: `/run/contextd/private/contextd-rgb-control.socket`

## Security Note

- **`RegisterController`** works from any process, but the hint's `pid` must be the calling process or one of its ancestors (the examples use their own PID; `0` also means "me"). Only the same user can remove it.
- **`SetLightingContext`** goes to the control socket, which only root and members of the `contextd-rgb` group can open (`sudo usermod -aG contextd-rgb $USER`, then log in again). If the administrator set `auth.authorized_units`, only those system services (and root) may write.

> **Note:** The daemon must be running for these examples to work.
