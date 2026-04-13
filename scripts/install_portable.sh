#!/bin/bash
set -e

# Configuration
IMAGE_NAME="game-linkd"
PROJECT_ROOT=$(git rev-parse --show-toplevel)
WORK_DIR="${PROJECT_ROOT}/${IMAGE_NAME}"
BINARY="${PROJECT_ROOT}/target/release/game_linkd"

echo "--- Building Game Linkd ---"
cargo build --release

echo "--- Detaching Existing Service (if any) ---"
sudo portablectl detach --now "${IMAGE_NAME}" || true

echo "--- Assembling Portable Image Tree ---"
mkdir -p "${WORK_DIR}/usr/bin"
mkdir -p "${WORK_DIR}/usr/lib/systemd/system"
mkdir -p "${WORK_DIR}/etc"

rm -f "${WORK_DIR}/usr/bin/${IMAGE_NAME}"
cp "${BINARY}" "${WORK_DIR}/usr/bin/${IMAGE_NAME}"

# Ensure the unit file exists in the image
# (We already created it, but the script can ensure it's synced)
cat <<EOF > "${WORK_DIR}/usr/lib/systemd/system/${IMAGE_NAME}.service"
[Unit]
Description=Game Linkd Daemon
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/game-linkd
Restart=always
RestartSec=5
RuntimeDirectory=game-linkd
BindReadOnlyPaths=/usr/lib /usr/lib64 /lib /lib64 /etc/ld.so.cache

[Install]
WantedBy=default.target
EOF

# Ensure os-release is present
cat <<EOF > "${WORK_DIR}/usr/lib/os-release"
ID=${IMAGE_NAME}
NAME="Game Linkd"
VERSION="0.1.0"
PORTABLE_SERVICE=1
EOF
ln -sf /usr/lib/os-release "${WORK_DIR}/etc/os-release"

echo "--- Attaching Portable Service ---"
# Attach the directory as a portable service
sudo portablectl attach --now --profile=trusted "${WORK_DIR}"

echo "--- Status ---"
sudo portablectl is-attached "${IMAGE_NAME}"
systemctl status "${IMAGE_NAME}" --no-pager
