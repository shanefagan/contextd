#!/bin/bash
set -e

echo "--- Building contextd ---"
cargo build --release

# Service name
NAME="contextd"
IMAGE_DIR="./${NAME}"
BIN_PATH="./target/release/${NAME}"

# Clean up previous image directory if it exists
# We use sudo because portablectl might have left root-owned files
sudo rm -rf "${IMAGE_DIR}"
mkdir -p "${IMAGE_DIR}/usr/bin"
mkdir -p "${IMAGE_DIR}/usr/lib/systemd/system"
mkdir -p "${IMAGE_DIR}/etc"

# Create os-release (required by portablectl)
cat <<EOF > "${IMAGE_DIR}/etc/os-release"
ID=contextd
NAME="Context Daemon Portable Image"
PRETTY_NAME="Context Daemon Portable Image"
EOF

# Copy binary
cp "${BIN_PATH}" "${IMAGE_DIR}/usr/bin/"

# Create service file
cat <<EOF > "${IMAGE_DIR}/usr/lib/systemd/system/${NAME}.service"
[Unit]
Description=Context Daemon
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/${NAME}
Restart=always
RestartSec=5
RuntimeDirectory=${NAME}
BindReadOnlyPaths=/usr/lib /usr/lib64 /lib /lib64 /etc/ld.so.cache /home
# Security Hardening
CapabilityBoundingSet=CAP_SYS_PTRACE CAP_DAC_READ_SEARCH
AmbientCapabilities=CAP_SYS_PTRACE CAP_DAC_READ_SEARCH
NoNewPrivileges=yes
ProtectSystem=strict
ProtectControlGroups=yes
ProtectKernelModules=yes
ProtectKernelTunables=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
MemoryDenyWriteExecute=yes

[Install]
WantedBy=default.target
EOF

echo "--- Detaching Existing Service (if any) ---"
sudo portablectl detach "${NAME}" --now --force || true

echo "--- Assembling Portable Image Tree ---"
# We just use the directory for now, portablectl can attach directories as images

echo "--- Attaching Portable Service ---"
sudo portablectl attach "${IMAGE_DIR}" --now --copy=symlink --profile=trusted

echo "--- contextd deployment complete ---"
echo "Check status with: systemctl status ${NAME}"
