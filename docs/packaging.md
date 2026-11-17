# Packaging & CI/CD

`contextd` is designed for high-availability and zero-dependency deployment across modern Linux distributions.

## 📦 Deployment Formats

### 1. systemd Portable Service (Recommended)
The primary deployment target is a **systemd portable service**. The image in `/opt/contextd` contains the daemon and its units:

| Unit | Purpose |
| :--- | :--- |
| `contextd.socket` / `contextd.service` | Core socket and daemon |
| `contextd-rgb-observer.socket`, `contextd-rgb-control.socket` / `contextd-rgb.service` | Lighting sockets and daemon |

- **Management**: `portablectl attach --now --enable --copy=symlink --profile=trusted /opt/contextd`
- `--enable` enables the socket units, so they come back after a reboot; the services start on first use.
- `portablectl` ships in `systemd-container` on Debian, Ubuntu and Fedora.

Some files live on the host, outside the image:

| File | Purpose |
| :--- | :--- |
| `/usr/lib/sysusers.d/contextd.conf` | Creates the `contextd-rgb` group that owns the control socket. Run `systemd-sysusers` before attaching. |
| `/usr/bin/openrgb-contextd-bridge` + its `.service` | Optional OpenRGB SDK bridge (not enabled by default) |
| `/usr/bin/contextctl` | CLI helper |
| `/usr/bin/contextd-focus` + `/usr/lib/systemd/user/contextd-focus.service` | Focus helper for graphical sessions; packages enable it globally (`systemctl --global enable`), so it starts at each user's next login |

### 2. Native Packages
The `.deb`, `.rpm` and Arch `PKGBUILD` install the same image and host files, and attach the image in their post-install scripts. All of them are assembled by `scripts/package-portable.sh`.

The image does not bundle libraries: its unit bind-mounts the host's `/usr/lib` (and friends) read-only, and the binary links only libc and libudev.

## 🚀 CI/CD Pipeline

The project uses GitHub Actions to automate the lifecycle of the daemon.

### Workflows:
1.  **Build & Test**: Runs on every push to `master` and all Pull Requests, across the whole workspace (daemon, client crate and bridge).
    *   `cargo fmt --all` (Style check)
    *   `cargo clippy --workspace --all-targets` (Linting)
    *   `cargo test --workspace` (Unit tests, plus end-to-end tests that run the daemon binary)
    *   `systemd-analyze verify` on every unit file
2.  **Security Audit**: `rustsec/audit-check`.
3.  **Package**: Builds the portable tarball, `.deb` and `.rpm` on every run.
4.  **Release**: On version tags (e.g., `v1.0.0`) only, uploads the packages to a GitHub Release. This is the only job with write access to the repository.

## 🛠️ Build Requirements
To build `contextd` from source, you need:
- Rust (Stable)
- `libudev-dev`
- `pkg-config`
- `systemd` (for `portablectl` validation)
