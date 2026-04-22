# Contextd Examples

This directory contains examples of how to interact with the `contextd` daemon from different programming languages.

Because `contextd` uses [Varlink](https://varlink.org) over standard Unix sockets, you can use any language that supports Unix domain sockets and JSON serialization.

## Available Examples

### 1. Python
Python is a great choice for scripting against `contextd` because it has built-in support for Unix sockets and JSON, requiring zero external dependencies.
- **`python/get_active_game.py`**: Queries the core daemon for the currently foregrounded game.
- **`python/subscribe_rgb.py`**: Subscribes to real-time ambient lighting updates from the RGB observer socket.

### 2. Shell
For simple queries in bash scripts, the `varlinkctl` tool (usually installed alongside varlink) allows one-line interactions.
- **`shell/get_diagnostics.sh`**: Uses `varlinkctl` to query system diagnostics.

### 3. Rust
While the `contextd` daemon itself is written in Rust and uses the `varlink` crate, you can also interact with it using nothing but the standard library and `serde_json`.
- **`rust/src/main.rs`**: A standard Cargo project showing how to connect to the Unix socket and parse a JSON response.
  *Run it with:* `cd rust && cargo run`

### 4. C
A minimal C example demonstrating how to use POSIX sockets to send a Varlink JSON request and read the response.
- **`c/get_active_game.c`**: Pure C with no external dependencies (uses standard `sys/socket.h` and `sys/un.h`).
  *Compile and run:* `gcc c/get_active_game.c -o get_active_game && ./get_active_game`

## Socket Paths
Remember the daemon exposes three interfaces:
1. **Core Socket**: `/run/contextd/public/contextd.socket`
2. **RGB Observer**: `/run/contextd/public/contextd-rgb-observer.socket`
3. **RGB Control**: `/run/contextd/private/contextd-rgb-control.socket`

> **Note:** The daemon must be running for these examples to work.
