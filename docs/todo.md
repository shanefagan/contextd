# Project TODO - Context Daemon

## Phase 5: Ecosystem & Context Maturity
- [x] **System Diagnostics**: Gathers RAM, CPU, GPU, and API info with long-term caching (5m).
- [x] **Dynamic Hardware Detection**: Use udev database for vendor-neutral GPU/peripheral reporting.
- [x] **Flatpak Integration**: Documented the "Socket Export" strategy in `examples/FLATPAK.md`.

## Phase 6: Packaging & CI
- [x] **GitHub Actions Workflow**: Automate build, test, and portable tree assembly.
- [x] **CI Robustness**: Fixed RPM packaging issues with proper source tarball exclusions.
- [x] **Release Automation**: Auto-generate `.tar.gz` and GitHub Releases on tags.
- [x] **Arch Linux PKGBUILD**: `packaging/arch/PKGBUILD` builds `contextd-git` (publish to the AUR still pending).

- [x] **Portable Service Hardening**: Refine the sandbox to work across different distro library versions.

- [x] **Logo & Identity**: Create a simple icon/logo for the project.
- [x] **Interface Refactoring & Cleanup**: Moved generated code to `OUT_DIR` and implemented `DynamicInterface` for `get_description` parity.


## Phase 7: Refinement & Security Hardening
- [x] **Systemd Peer Validation**: Uses `SO_PEERCRED` to identify calling services and apply granular access control based on systemd units.
- [x] **Configuration Overrides**: Implemented `src/config.rs` with `/etc/contextd/config.toml` support for TTLs and blacklisting.
- [x] **Directory Structure Cleanup**: Refactored logic into `server.rs`, `auth.rs`, and centralized detector management.
- [x] **Code review fixes (v0.10.0)**: Fail-closed config, connection limits, socket activation, caller-bound controller hints, matrix overflow, Lutris/Heroic detection, OpenRGB bridge protocol.
- [ ] **Privacy Masking**: Implement logic to mask specific apps/devices from public Varlink calls via `config.toml` (the blacklist covers part of this).
- [ ] **Per-user core service**: Move game detection to a `systemctl --user` service so it needs no capabilities and users cannot see each other's games. Changes the core socket path.
- [x] **Foreground game**: `contextd-focus` session helper (X11/XWayland, Hyprland, Sway).
- [x] **Push updates**: `SubscribeRunningGames`, `SubscribeDevices`.
- [x] **More launchers**: Steam shortcuts, Bottles, itch.io.
- [x] **Real RGB device list** from OpenRGB's udev rules.
- [x] **Compact matrix encoding** (`packed_matrix`).
- [ ] **Native Wayland focus on KDE/GNOME**: needs a KWin script or GNOME extension to report focus.
- [ ] **Lighting priorities**: Add a source name / priority to `SetLightingContext` so two controllers do not fight (last writer wins today).
- [ ] **Expanded Launcher Support**: Add Legendary (Epic) and Minigalaxy (GOG) manifests.
- [ ] **Arch Linux PKGBUILD**: Finalize the AUR package for the 1.0 release.
