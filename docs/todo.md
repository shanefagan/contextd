# Project TODO - Context Daemon

## Phase 1: Foundation
- [x] Finalize Varlink Interface Definition (`.varlink` file).
- [x] Create generic `GameDetector` and `HardwareDetector` traits.
- [x] Implement `GameManager` and `HardwareManager` orchestrators.
- [x] Initialize Rust project with foundational crates (`serde`, `varlink`, `udev`).
- [x] Implement Varlink server loop.

## Phase 2: Session Detection
- [x] Implement Steam game detection (parsing `.acf` and `libraryfolders.vdf`).
- [x] Implement Heroic Games Launcher support (Epic, GOG, Amazon).
- [x] Implement Lutris detector (SQLite `pga.db` parsing).
- [ ] Implement generic process-based detection (searching `/proc` for non-launcher apps).
- [ ] Add real-time event subscription for session state changes.

## Phase 3: Hardware Inventory
- [x] Implement udev-based device discovery.
- [x] Filter for relevant classes (Keyboards, Mice, Gamepads).
- [x] Refine "real" device detection (ignore dongles/hubs where possible).
- [x] Add uaccess/permission check logic.
- [ ] Implement hotplug notification logic (udev monitoring).
- [x] Export device list via Varlink.

## Phase 4: Deployment & Polish
- [x] Create a CLI client for testing the Varlink interface (`scripts/contextctl.sh`).
- [x] Implement `portablectl` image assembly and install script.
- [x] Configure socket permissions (0666) for userspace access.
- [x] Bind-mount host paths (/home, /usr/lib) for portable service compatibility.
- [x] Bundle `os-release` in the portable image tree.
- [ ] Comprehensive documentation of the interface methods.
- [ ] Performance optimization (minimize resource usage during detection).
