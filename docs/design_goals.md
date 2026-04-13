# Design Goals - Context Daemon (`contextd`)

Context Daemon is a lightweight Linux utility designed to bridge the gap between running games/applications and userspace tools (like RGB controllers, profile switchers, or system monitors). It provides a generic, non-sensitive interface via Varlink to query the current system state regarding active software sessions and hardware.

## Core Objectives

### 1. Unified Session Detection
- Detect when a high-intensity game or application is running regardless of the launcher (Steam, Lutris, Heroic, Flatpaks, or standalone binaries).
- Provide metadata about the running session (Title, AppID if applicable, Process ID).
- Avoid complex "integrations" and focus on reporting raw state.

### 2. Hardware Inventory
- Expose a list of connected interaction hardware (Keyboards, Mice, Controllers).
- Provide basic device identification (Vendor ID, Product ID, Name, Bus Type).
- Monitor for hotplug events to keep the inventory up-to-date.

### 3. Security & IPC Model
- **Root-level Daemon**: Runs as root to have unrestricted access to `/proc`, `/home` manifests, and `udev`.
- **Public Socket**: Exposes a `0666` permission socket at `/run/contextd/contextd.socket`.
- **PackageKit-style**: Similar to PackageKit or systemd-networkd, it allows unprivileged user applications to query system-wide context without requiring `sudo` or complex DBus permissions.
- **Privacy-First**: Only exposes metadata about apps and hardware; no PII or telemetry.

### 4. Linux-Centric Design
- Leverage native Linux APIs (udev, procfs) to provide the most efficient implementation.
- Focus on modern Linux standards (Varlink, Systemd Portable Services).

### 5. Deployment via Portable Services
- Support `systemd-portabled` for isolated, distro-agnostic deployment.
- Ship as a self-contained OS tree for easy "attaching" to any modern host.

## Architecture

- **`contextd` Daemon**: The core service running in the background.
- **Varlink Interface**: The primary way for clients to interact with the daemon (`io.github.contextd`).
- **Detectors**: Modular components for identifying software (Steam, Heroic, Lutris detectors).
- **Inventory**: A hardware tracker using `udev`.
