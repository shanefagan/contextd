# Design Goals - Context Daemon (`contextd`)

Context Daemon is a lightweight Linux utility designed to bridge the gap between running games/applications and userspace tools (like RGB controllers, profile switchers, or system monitors). It provides a generic, non-sensitive interface via Varlink to query the current system state regarding active software sessions and hardware.

## Core Objectives

### 1. Unified Session Detection
- Detect when a high-intensity game or application is running through supported launchers (Steam, Lutris, Heroic).
- Provide metadata about the running session (Title, AppID if applicable, Process ID).
- Avoid complex "integrations" and focus on reporting raw state.

### 2. Hardware Inventory
- Expose a list of connected interaction hardware (Keyboards, Mice, Controllers).
- Separate RGB lighting controllers and fans into a dedicated "Aesthetic" inventory to avoid cluttering gaming gear lists.
- **Poll-on-Demand**: Uses a cached results model (TTL-based) rather than real-time hotplug monitoring. This keeps the daemon lightweight and removes the need for high-privilege `CAP_NET_ADMIN` permissions.

### 3. System Sanity & Diagnostics
- Provide a "Bug Report Readiness" check for Linux gaming.
- Expose hardware specs (RAM, CPU) and platform-level support (Vulkan, OpenGL).
- Surface environment context (Snap/Flatpak) to help developers and maintainers troubleshoot performance or library issues.

### 3. Security & IPC Model
- **Principle of Least Privilege**: Although the daemon is initialized by systemd, it drops almost all root privileges via the Portable Service sandbox. It retains only `CAP_SYS_PTRACE` (for process scanning) and `CAP_DAC_READ_SEARCH` (for reading game manifests in `/home`).
- **Strict Sandboxing**: Utilizes `ProtectSystem=strict`, `MemoryDenyWriteExecute=yes`, and other systemd hardenings to ensure the daemon cannot be easily compromised.
- **Public Sockets**: Exposes `0666` permission sockets at `/run/contextd/public/contextd.socket` (Core), `/run/contextd/public/contextd-rgb-observer.socket` (RGB Observer), and `/run/contextd/private/contextd-rgb-control.socket` (RGB Control). Both daemons share the same dynamic user allowing them to coordinate in `/run/contextd`.

### 4. Linux-Centric Design
- Leverage native Linux APIs (udev, procfs) to provide the most efficient implementation.
- Focus on modern Linux standards (Varlink, Systemd Portable Services).

## Philosophy
- **Context NOT Configuration**: `contextd` is a context provider. It reports *what* is happening. It does not attempt to configure hardware, map keys, or manage lighting. Configuration should be handled by specialized client-side tools using the context provided here.
- **Pull-over-Push**: For hardware state, we prefer simple polling with optimized caching. This avoids the fragility of system-wide hotplug listeners in a containerized world.

## Future: Flatpak Portal Strategy
To support sandboxed applications (like OBS or Flatpak-based hardware drivers), `contextd` targets a D-Bus "Portal Bridge" model:
- **Metadata Proxy**: Proxy Varlink calls to D-Bus for apps that cannot access Unix sockets.
- **FD Passing**: Provide a mechanism to open `/dev/hidraw` nodes on behalf of sandboxed apps and pass the File Descriptor over D-Bus, eliminating the need for wide `--device=all` Flatpak permissions.
- **User Consent**: Integrate with desktop portals to provide "Allow this app to see your hardware" prompts.
