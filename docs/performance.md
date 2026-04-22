# Performance Analysis

`contextd` is designed to be a lightweight background service with minimal impact on system resources.

## ⚡ Caching Strategy

The daemon uses a "Poll-on-Demand" strategy with Time-To-Live (TTL) caching to avoid redundant work while ensuring data is reasonably fresh.

| Category | TTL (Default) | Impact of Refresh |
| :--- | :--- | :--- |
| **Games** | 5 Seconds | **Medium**: Reads `/proc` and launcher manifests. |
| **Hardware** | 10 Seconds | **Low**: Enumerates udev nodes. |
| **Diagnostics** | 300 Seconds | **High**: Probes GPU, RAM, and libraries. |

### Implementation Details:
1.  **Zero-Background Work**: The daemon does not monitor udev or processes in the background. Scans only happen when a client requests data AND the cache has expired.
2.  **Threaded Server**: Uses a thread pool to handle multiple Varlink connections concurrently without blocking the main event loop.
3.  **Memory Management**: Minimal use of long-lived allocations. Caches are cleared or overwritten on TTL expiry.

## 🔍 Efficiency Optimizations

*   **Filtered procfs Scanning**: When identifying active games, the daemon only inspects environments of processes that match known launcher patterns, avoiding a full scan of all running PIDs where possible.
*   **Deduplicated Hardware**: Complex udev hierarchies (like a multi-function gaming keyboard) are merged into a single logical `Device` entry to reduce IPC payload size.
