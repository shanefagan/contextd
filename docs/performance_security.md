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

### Proposed Optimizations
1. **Caching (Quick Win)**:
   - Cache results for 5-10 seconds.
   - Background thread refreshes the cache so the Varlink reply is always instantaneous.
2. **Event-Driven (Long Term)**:
   - Use `inotify` to watch Steam/Heroic/Lutris manifest folders. Only re-scan when a file changes.
   - Use `udev` monitoring instead of enumeration for device changes.
3. **Process Pulse**:
   - Instead of scanning all of `/proc`, use a background thread that scans once per second (or more frequent if the "monitor" command is active).

---

## 2. Security

### Current State: Full Root
The daemon runs as `User=root` within the portable service. This is necessary for:
- Reading `/proc/*/environ` (requires `CAP_SYS_PTRACE`).
- Accessing other users' `/home` game manifests.
- Managing the `/run/contextd` directory and socket permissions.

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
   - Change group of `/run/contextd` to `gaming` or similar, or use an ACL.

---

## 3. Implementation Plan (Phase 5)

1. **Implement Caching**: Move from "Request -> Scan" to "Request -> Cached Data".
2. **Background Watcher**: Add a background thread that updates the `GameManager` state periodically.
3. **Cap-Limited Service**: Update the `.service` file to drop unnecessary root privileges.
