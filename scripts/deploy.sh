#!/bin/bash
# Builds contextd and installs it on this machine as a portable service in
# /opt/contextd, plus the host-side pieces (rgb group, OpenRGB bridge).
set -euo pipefail

NAME="contextd"
INSTALL_DIR="/opt/${NAME}"
PORTABLE_UNITS="contextd.service contextd.socket contextd-rgb.service contextd-rgb-observer.socket contextd-rgb-control.socket"

echo "--- Building workspace (release) ---"
cargo build --release --workspace

echo "--- Stopping existing service if running ---"
sudo portablectl detach --now --enable "${NAME}" || true
sudo rm -f /etc/systemd/system/contextd.service /etc/systemd/system/contextd-rgb.service

echo "--- Preparing portable root in ${INSTALL_DIR} ---"
sudo mkdir -p "${INSTALL_DIR}"/{usr/bin,usr/lib/systemd/system,etc,home,sys,run,proc}
sudo install -m 755 target/release/contextd "${INSTALL_DIR}/usr/bin/"
for unit in ${PORTABLE_UNITS}; do
    sudo install -m 644 "packaging/${unit}" "${INSTALL_DIR}/usr/lib/systemd/system/"
done
cat <<EOF | sudo tee "${INSTALL_DIR}/etc/os-release" > /dev/null
ID=contextd
NAME="Context Daemon Portable Root"
PRETTY_NAME="Context Daemon Portable Root"
EOF

echo "--- Host-side files ---"
# The control socket's group must exist before its socket unit starts.
sudo install -D -m 644 packaging/sysusers.d/contextd.conf /etc/sysusers.d/contextd.conf
sudo systemd-sysusers /etc/sysusers.d/contextd.conf
sudo install -m 755 target/release/openrgb-contextd-bridge /usr/local/bin/
sed 's|/usr/bin/openrgb-contextd-bridge|/usr/local/bin/openrgb-contextd-bridge|' \
    packaging/openrgb-contextd-bridge.service \
    | sudo tee /etc/systemd/system/openrgb-contextd-bridge.service > /dev/null
sudo install -m 755 scripts/contextctl.sh /usr/local/bin/contextctl
sudo install -m 755 target/release/contextd-focus /usr/local/bin/
sed 's|/usr/bin/contextd-focus|/usr/local/bin/contextd-focus|' \
    packaging/user/contextd-focus.service \
    | sudo tee /etc/systemd/user/contextd-focus.service > /dev/null
# Start the focus helper in every user's graphical session.
sudo systemctl --global enable contextd-focus.service
sudo systemctl daemon-reload

echo "--- Attaching portable service from ${INSTALL_DIR} ---"
# /opt avoids the hidden-namespace restrictions of /usr/lib/portables.
# --enable makes the sockets start at boot; services start on first use.
sudo portablectl attach --now --enable --copy=symlink --profile=trusted "${INSTALL_DIR}"

echo "--- Done! ---"
echo "Status:          systemctl status contextd.socket contextd-rgb-control.socket"
echo "List games:      contextctl list-games"
echo "Set lighting:    add yourself to the contextd-rgb group, then: contextctl rgb-set 255 0 0"
echo "Focus helper:    systemctl --user daemon-reload; systemctl --user start contextd-focus.service"
echo "OpenRGB bridge:  sudo systemctl enable --now openrgb-contextd-bridge.service"
