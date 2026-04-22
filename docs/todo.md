# Project TODO - Context Daemon

## Phase 5: Ecosystem & Context Maturity
- [x] **System Diagnostics**: Gathers RAM, CPU, GPU, and API info with long-term caching (5m).
- [x] **Dynamic Hardware Detection**: Use udev database for vendor-neutral GPU/peripheral reporting.
- [x] **Flatpak Integration**: Documented the "Socket Export" strategy in `examples/FLATPAK.md`.

## Phase 6: Packaging & CI
- [x] **GitHub Actions Workflow**: Automate build, test, and portable tree assembly.
- [x] **CI Robustness**: Fixed RPM packaging issues with proper source tarball exclusions.
- [x] **Release Automation**: Auto-generate `.tar.gz` and GitHub Releases on tags.
- [ ] **Arch Linux PKGBUILD**: Create/Update an AUR package for `contextd-git`.

- [x] **Portable Service Hardening**: Refine the sandbox to work across different distro library versions.

- [x] **Logo & Identity**: Create a simple icon/logo for the project.
- [x] **Interface Refactoring & Cleanup**: Moved generated code to `OUT_DIR` and implemented `DynamicInterface` for `get_description` parity.


## Phase 7: Refinement & Security Hardening
- [ ] **Systemd Peer Validation**: Use `SO_PEERCRED` to identify calling services and apply granular access control based on systemd units.
- [x] **Configuration Overrides**: Implemented `src/config.rs` with `/etc/contextd/config.toml` support for TTLs and blacklisting.
- [ ] **Arch Linux PKGBUILD**: Finalize the AUR package for the 1.0 release.
