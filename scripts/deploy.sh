#!/bin/bash
# Universal Context Daemon Installer (Portable Mode)
set -e

NAME="contextd"
INSTALL_DIR="/opt/${NAME}"
SERVICE_FILE="packaging/contextd.service"

echo "--- Building contextd (Release) ---"
cargo build --release

echo "--- Stopping existing service if running ---"
sudo portablectl detach "${NAME}" --now || true

echo "--- Preparing Portable Root in ${INSTALL_DIR} ---"
sudo mkdir -p "${INSTALL_DIR}/usr/bin"
sudo mkdir -p "${INSTALL_DIR}/usr/lib/systemd/system"
sudo mkdir -p "${INSTALL_DIR}/etc"
sudo mkdir -p "${INSTALL_DIR}/home"
sudo mkdir -p "${INSTALL_DIR}/sys"
sudo mkdir -p "${INSTALL_DIR}/run"
sudo mkdir -p "${INSTALL_DIR}/proc"

sudo install -m 755 target/release/contextd "${INSTALL_DIR}/usr/bin/"
sudo install -m 644 packaging/contextd.service "${INSTALL_DIR}/usr/lib/systemd/system/"
sudo install -m 644 packaging/contextd-rgb.service "${INSTALL_DIR}/usr/lib/systemd/system/"

# Create os-release
cat <<EOF | sudo tee "${INSTALL_DIR}/etc/os-release" > /dev/null
ID=contextd
NAME="Context Daemon Portable Root"
PRETTY_NAME="Context Daemon Portable Root"
EOF

echo "--- Attaching Portable Service from /opt ---"
# We use /opt to bypass the hidden namespace restrictions of /usr/lib/portables
sudo portablectl attach "${INSTALL_DIR}" --now --copy=symlink --profile=trusted

echo "--- Done! ---"
echo "Check status with: systemctl status contextd"
echo "List games with: ./scripts/contextctl.sh list-games"
