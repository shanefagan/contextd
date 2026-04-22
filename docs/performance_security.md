# Security & Performance Analysis - `contextd`

This document evaluates the current state of the Context Daemon and proposes optimizations to ensure it remains lightweight and secure.

## 1. Performance

### Current State: "Poll-on-Demand"
Currently, `contextd` performs fresh system scans every time a Varlink method is called.

| Operation | Action | Impact |
| :--- | :--- | :--- |
| `ListInstalledGames` | Scans `/home/*/` for Steam ACF, Heroic JSON, and Lutris SQL | **High (IO)**. Parsing many small files or large SQLite DBs on every call is inefficient. |
| `GetActiveGame` | Iterates over ALL `/proc/[pid]/environ` files | **Critical (CPU/IO)**. On a busy system, reading thousands of environ files is slow and generates unnecessary load. |
| `ListDevices` | Scans the `input` subsystem via udev | **Low**. Udev handles the heavy lifting, but we still rebuild the list. |

### 2. Implementation Efficiency
   - **Zero-Poll Hardware**: Instead of a background monitor thread, hardware is enumerated only on request and cached for 10 seconds. This avoids context switching and kernel interrupts for hardware events the user might not care about.
   - **Optimized Game Detection**: Instead of scanning all of `/proc` continuously, the daemon uses a 5-second TTL cache for active game results.

---

## 2. Security

### Current State: Dynamic User with Capabilities
The daemon runs as a `DynamicUser` (defaulting to the `contextd` username) within the portable service. It uses systemd capabilities to elevate privileges only where strictly necessary:
- Reading `/proc/*/environ` (requires `CAP_SYS_PTRACE`).
- Accessing other users' `/home` game manifests (requires `CAP_DAC_READ_SEARCH`).
- Sharing the `/run/contextd` directory between the Core and RGB services.

### Potential Risks
- **Information Leak**: Any local user can see what games/apps another user is running via the `0666` socket.
- **Attack Surface**: A bug in the detector (e.g., malformed JSON parsing) could lead to root code execution.

### Tighter Sandboxing
We should leverage systemd security features to limit the daemon's powers:

1. **Capability Dropping**:
   - `CapabilityBoundingSet=CAP_SYS_PTRACE CAP_DAC_READ_SEARCH`
   - Use `AmbientCapabilities` to run as a non-root user that still has these specific caps.
2. **Filesystem Isolation**:
   - `ProtectSystem=strict`
   - `ProtectHome=read-only` (Already implemented via `BindReadOnlyPaths`)
   - `PrivateDevices=no` (We need device nodes for udev/peripherals)
   - `NoNewPrivileges=yes`
3. **Socket Permissions**:
   - For ease of integration and zero-configuration use, the daemon exposes `0666` public sockets. Both the Core service and RGB service share the same `DynamicUser` to cleanly manage files in `/run/contextd`.
4. **Resilient Subscriptions**:
   - The RGB daemon uses bounded (`mpsc::sync_channel`) channels with non-blocking sends (`try_send`) for lighting updates. This prevents a slow or malicious client on the observer socket from exhausting daemon memory by buffering unbounded events.

---

## 3. Implementation Status (Current)

1.  **DONE**: Implemented TTL caching for games (5s) and hardware (10s).
2.  **DONE**: Hardened `systemd` portable configuration with capability bounding (`CAP_SYS_PTRACE`, `CAP_DAC_READ_SEARCH`) and `DynamicUser`.
3.  **DONE**: Dropped real-time `udev` monitoring in favor of zero-overhead polling to keep the daemon lightweight and robust.
4.  **DONE**: Implemented long-term caching for System Diagnostics (5-minute refresh) to avoid redundant hardware probing.
5.  **DONE**: RGB daemon split into a separate service (`contextd-rgb.service`) that shares the same dynamic user to avoid permission conflicts.
