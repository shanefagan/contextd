#!/bin/bash
# Assembles the portable service tree into a directory
set -e

NAME="contextd"
BUILD_DIR=${1:-"dist"}
BIN_PATH=${2:-"target/release/contextd"}

echo "Assembling portable tree in ${BUILD_DIR}..."

mkdir -p "${BUILD_DIR}/usr/bin"
mkdir -p "${BUILD_DIR}/usr/lib/systemd/system"
mkdir -p "${BUILD_DIR}/etc"
mkdir -p "${BUILD_DIR}/home"
mkdir -p "${BUILD_DIR}/sys"
mkdir -p "${BUILD_DIR}/run"
mkdir -p "${BUILD_DIR}/proc"

cp "${BIN_PATH}" "${BUILD_DIR}/usr/bin/"
cp "packaging/contextd.service" "${BUILD_DIR}/usr/lib/systemd/system/"

# Create os-release for identification
cat <<EOF > "${BUILD_DIR}/etc/os-release"
ID=contextd
NAME="Context Daemon Portable Root"
PRETTY_NAME="Context Daemon Portable Root"
EOF

echo "Done."
