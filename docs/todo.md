# Project TODO - Context Daemon

## Phase 5: Ecosystem & Context Maturity
- [x] **System Diagnostics**: Gathers RAM, CPU, GPU, and API info with long-term caching (5m).
- [x] **Dynamic Hardware Detection**: Use udev database for vendor-neutral GPU/peripheral reporting.
- [ ] **Flatpak Integration**: Document and test the "Socket Export" strategy for allowing Flatpak-based OBS to query the daemon.

## Phase 6: Packaging & CI
- [x] **GitHub Actions Workflow**: Automate build, test, and portable tree assembly.
- [x] **CI Robustness**: Fixed RPM packaging issues with proper source tarball exclusions.
- [x] **Release Automation**: Auto-generate `.tar.gz` and GitHub Releases on tags.
- [ ] **Arch Linux PKGBUILD**: Create/Update an AUR package for `contextd-git`.

- [x] **Portable Service Hardening**: Refine the sandbox to work across different distro library versions.

- [x] **Logo & Identity**: Create a simple icon/logo for the project.

