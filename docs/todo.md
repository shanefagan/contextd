# Project TODO - Game Linkd

## Phase 1: Foundation
- [x] Finalize Varlink Interface Definition (`.varlink` file).
- [x] Create generic `GameDetector` and `HardwareDetector` traits.
- [x] Implement `GameManager` and `HardwareManager` orchestrators.
- [x] Initialize Rust project with foundational crates (`serde`, `varlink`, `udev`).
- [ ] Implement Varlink server loop.

## Phase 2: Game Detection
- [ ] Implement Steam game detection (parsing `.acf` and `libraryfolders.vdf`).
- [ ] Implement generic process-based detection (searching `/proc`).
- [ ] Implement Lutris/Heroic detectors.
- [ ] Add real-time event subscription for game state changes.

## Phase 3: Hardware Inventory
- [ ] Implement udev-based device discovery.
- [ ] Filter for relevant classes (Keyboards, Mice, Gamepads).
- [ ] Implement hotplug notification logic.
- [ ] Export device list via Varlink.

## Phase 4: Polish & Integration
- [ ] Create a CLI client for testing the Varlink interface.
- [x] Implement `portablectl` image assembly and install script.
- [ ] Comprehensive documentation of the interface methods.
- [ ] Performance optimization (minimize resource usage during detection).
