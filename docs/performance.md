# Performance Analysis

`contextd` is designed to be a lightweight background service with minimal impact on system resources.

## ⚡ Caching Strategy

The daemon uses a "Poll-on-Demand" strategy with Time-To-Live (TTL) caching to avoid redundant work while ensuring data is reasonably fresh. All TTLs can be changed under `[ttls]` in `/etc/contextd/config.toml`.

| Category | TTL (Default) | Impact of Refresh |
| :--- | :--- | :--- |
| **Running games** | 5 Seconds | **Medium**: One pass over `/proc`, reading process environments. |
| **Installed games** | 60 Seconds | **Medium**: Reads Steam manifests, the Lutris database and Heroic's JSON files. |
| **Hardware** | 10 Seconds | **Low**: Enumerates udev nodes. |
| **Diagnostics** | 300 Seconds | **High**: Probes GPU, RAM, and libraries. Not collected until first requested. |

### Implementation Details:
1.  **Zero-Background Work**: Nothing is monitored unless a client asks. Plain calls scan only when the cache has expired. With socket activation, neither daemon even runs until a client first connects.
2.  **Subscriptions**: While anyone subscribes to running games, `/proc` is rescanned about once a second, shared by all subscribers through the cache; focus reports wake subscribers without a scan. The first device subscription adds one inotify watch on `/run/udev/data`, so the device list is rescanned only when udev's database changes (debounced per plug event). Subscribers only receive a message when their list changed.
3.  **Thread per connection**: Each connection is served on its own small-stack thread, so long-lived subscriptions and idle clients never block other callers. Connection counts are capped per socket and per user, and glibc malloc arenas are capped at 2 to keep RSS flat.
4.  **Memory Management**: Minimal use of long-lived allocations. Caches are overwritten on TTL expiry, and incoming messages are size-capped.
5.  **Lighting fan-out**: A lighting update increments a generation counter and wakes subscribers; each subscriber sends the newest state when it is ready, so a slow subscriber never builds a queue. The matrix is stored once as bytes; a 128×128 frame is 87 KB as `packed_matrix` versus 528 KB as a JSON color array.

## 🔍 Efficiency Optimizations

*   **Single procfs pass**: Running-game detection reads `/proc` once for all launchers and keeps only the few environment variables the launchers set (`SteamAppId`, `SteamGameId`, `HEROIC_APP_NAME`, `LUTRIS_GAME_UUID`, `GAME_NAME`, `WINEPREFIX`). Executable, working directory and first argument are read only when Bottles or itch.io games are installed, since those launchers tag nothing.
*   **Installed list reuse**: Running-game detection looks games up in the cached installed list instead of rescanning launcher files.
*   **Grouped Hardware**: Nodes that belong to one physical device (a keyboard's several event and hidraw nodes) are merged into a single `Device` entry, with every node listed in `paths`.
