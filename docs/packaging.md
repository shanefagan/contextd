# Packaging & CI/CD

`contextd` is designed for high-availability and zero-dependency deployment across modern Linux distributions.

## 📦 Deployment Formats

### 1. systemd Portable Service (Recommended)
The primary deployment target is a **systemd portable service**. This encapsulates the daemon and its minimal dependencies into a self-contained OS tree.
- **Location**: `/opt/contextd`
- **Management**: `portablectl attach /opt/contextd`

### 2. Native Packages
Native packages (like the Arch Linux AUR package) typically automate the build and attachment of the portable image to ensure consistent behavior across different library versions on the host.

## 🚀 CI/CD Pipeline

The project uses GitHub Actions to automate the lifecycle of the daemon.

### Workflows:
1.  **Build & Test**: Runs on every push to `main` and all Pull Requests.
    *   `cargo fmt` (Style check)
    *   `cargo clippy` (Linting)
    *   `cargo test` (Unit/Integration tests)
2.  **Release Assembly**: Runs on version tags (e.g., `v1.0.0`).
    *   Assembles the portable service OS tree.
    *   Generates source tarballs.
    *   Uploads artifacts to a GitHub Release.

## 🛠️ Build Requirements
To build `contextd` from source, you need:
- Rust (Stable)
- `libudev-dev`
- `pkg-config`
- `systemd` (for `portablectl` validation)
