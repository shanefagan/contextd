#!/bin/bash
# Assembles the release tree:
#   ${BUILD_DIR}/contextd/  portable service image (attach with portablectl)
#   ${BUILD_DIR}/host/      files for the host: sysusers group, OpenRGB
#                           bridge binary and unit, contextctl
set -euo pipefail

BUILD_DIR=${1:-"dist"}
TARGET_DIR=${2:-"target/release"}
ROOT="${BUILD_DIR}/contextd"
HOST="${BUILD_DIR}/host"
PORTABLE_UNITS="contextd.service contextd.socket contextd-rgb.service contextd-rgb-observer.socket contextd-rgb-control.socket"

echo "Assembling portable image in ${ROOT}..."
mkdir -p "${ROOT}"/{usr/bin,usr/lib/systemd/system,etc,home,sys,run,proc}
install -m 755 "${TARGET_DIR}/contextd" "${ROOT}/usr/bin/"
for unit in ${PORTABLE_UNITS}; do
    install -m 644 "packaging/${unit}" "${ROOT}/usr/lib/systemd/system/"
done
cat <<EOF > "${ROOT}/etc/os-release"
ID=contextd
NAME="Context Daemon Portable Root"
PRETTY_NAME="Context Daemon Portable Root"
EOF

echo "Assembling host files in ${HOST}..."
install -D -m 644 packaging/sysusers.d/contextd.conf "${HOST}/usr/lib/sysusers.d/contextd.conf"
install -D -m 755 "${TARGET_DIR}/openrgb-contextd-bridge" "${HOST}/usr/bin/openrgb-contextd-bridge"
install -D -m 644 packaging/openrgb-contextd-bridge.service \
    "${HOST}/usr/lib/systemd/system/openrgb-contextd-bridge.service"
install -D -m 755 scripts/contextctl.sh "${HOST}/usr/bin/contextctl"

cat <<'EOF' > "${BUILD_DIR}/INSTALL.txt"
1. sudo cp -r contextd /opt/contextd
2. sudo cp -r host/usr/. /usr/
3. sudo systemd-sysusers contextd.conf
4. sudo portablectl attach --now --enable --copy=symlink --profile=trusted /opt/contextd
5. Optional: sudo systemctl enable --now openrgb-contextd-bridge.service
Users who may change lighting need to be in the contextd-rgb group.
EOF

echo "Done."
