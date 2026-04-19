# Packaging & CI Strategy - `contextd`

This document outlines the workflow for building, assemling, and distributing `contextd` as a hybrid portable service.

## 1. Build Pipeline (GitHub Actions)

Every push to `master` and every `tag` triggers the following pipeline:

### Workflow Steps:
1. **Lint & Test**: Run `cargo fmt`, `cargo clippy`, and `cargo test`.
2. **Multi-Arch Build** (Future): Cross-compile for `x86_64` and `aarch64` (Steam Deck / ARM handhelds).
3. **Assemble Portable Tree**:
   - Create a directory structure matching the portable service spec.
   - Copy the binary to `usr/bin/`.
   - Copy the unit files (Core and RGB) to `usr/lib/systemd/system/`.
   - Ensure `usr/lib/os-release` is present.
4. **Archive**: Create `contextd-vX.Y.Z-x86_64.tar.gz`.
5. **Release**: Upload to GitHub Releases for tagged commits.

## 2. Distro Package Design

The goal is to maintain the `portablectl` isolation while using native package managers for delivery.

### Generic Post-Install Script:
```bash
# 1. Unpack the tree to /usr/lib/portables/contextd
# 2. Attach the service
portablectl attach --now --copy=symlink /usr/lib/portables/contextd
```

### Target Repositories:
- **Arch Linux (AUR)**: A `PKGBUILD` that pulls the GitHub tarball and handles the `portablectl` attachment.
- **Fedora (COPR)**: An `.rpm` spec that bundles the portable tree.
- **Debian/Ubuntu (PPA)**: A `.deb` package focusing on LTS compatibility.

## 3. Automation Challenges
- **Sudo requirement**: `portablectl` requires root. Packages must handle this via standard `postinst` scripts.
- **Library Compatibility**: Since the portable service uses host libraries (symlink mode), we must ensure the binary is linked against a reasonably old `glibc` (e.g., using an older build runner like Ubuntu 22.04).
