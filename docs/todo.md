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
- [x] Implement generic process-based detection (searching `/proc` for non-launcher apps).

## Phase 3: Hardware Inventory
- [x] Implement udev-based device discovery.
- [x] Filter for relevant classes (Keyboards, Mice, Gamepads).
- [x] Separate RGB/Aesthetic devices from Gaming Gear.
- [x] Add uaccess/permission check logic.
- [x] Implement 10-second TTL caching for sub-microsecond IPC response.
- [x] Export device list via Varlink.

## Phase 4: Deployment & Polish
- [x] Create a CLI client for testing the Varlink interface (`scripts/contextctl.sh`).
- [x] Implement `portablectl` image assembly and install script.
- [x] Configure socket permissions (0666) for userspace access.
- [x] Hardened systemd sandbox with capability dropping.
- [x] Add unit tests for core manager logic.
- [x] Comprehensive documentation and design goals.

## Phase 5: Ecosystem & Context Maturity
- [ ] **Flatpak Integration**: Document and test the "Socket Export" strategy for allowing Flatpak-based OBS to query the daemon.

## Phase 6: Packaging & CI
- [ ] **GitHub Actions Workflow**: Automate build, test, and portable tree assembly.
- [ ] **Release Automation**: Auto-generate `.tar.gz` and GitHub Releases on tags.
- [ ] **Arch Linux PKGBUILD**: Create an AUR package for `contextd-git`.
- [ ] **Portable Service Hardening**: Refine the sandbox to work across different distro library versions.
- [ ] **Logo & Identity**: Create a simple icon/logo for the project.
