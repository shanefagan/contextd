# Design Goals - Game Linkd

Game Linkd is a lightweight Linux daemon designed to bridge the gap between running games and userspace applications (like RGB controllers, profile switchers, or system monitors). It provides a generic, non-sensitive interface via Varlink to query the current system state regarding gaming activity and hardware.

## Core Objectives

### 1. Unified Game Detection
- Detect when a game is running regardless of the launcher (Steam, Lutris, Heroic, Flatpaks, or standalone binaries).
- Provide metadata about the running game (Title, AppID if applicable, Process ID).
- Avoid complex "integrations" and focus on reporting state.

### 2. Hardware Inventory
- Expose a list of connected gaming-relevant hardware (Keyboards, Mice, Controllers).
- Provide basic device identification (Vendor ID, Product ID, Name, Bus Type).
- Monitor for hotplug events to keep the inventory up-to-date.

### 3. Privacy-First & Generic
- Only expose non-sensitive information.
- No tracking of user behavior beyond what is necessary for profile switching.
- Standardized Varlink interface that any client can implement.

### 4. Linux-Centric Design
- Leverage native Linux APIs (udev, procfs, D-Bus where helpful) to provide the most efficient implementation.
- Focus on modern Linux standards (Varlink).

### 5. Deployment via Portable Services
- Support `systemd-portabled` for isolated, distro-agnostic deployment.
- Ship as a self-contained OS tree or disk image for easy "attaching" to the host.

## Architecture

- **`game-linkd` Daemon**: The core service running in the background.
- **Varlink Interface**: The primary way for clients to interact with the daemon.
- **Detectors**: Modular components for identifying games (Steam Watcher, Process Watcher).
- **Inventory**: A hardware tracker using `udev`.
