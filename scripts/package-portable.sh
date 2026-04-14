#!/bin/bash
# Assembles a self-contained portable service tree
set -e

NAME="contextd"
BUILD_DIR=${1:-"dist"}
BIN_PATH=${2:-"target/release/contextd"}

echo "Assembling universal portable tree in ${BUILD_DIR}..."

# Create directory structure
mkdir -p "${BUILD_DIR}/usr/bin"
mkdir -p "${BUILD_DIR}/usr/lib/systemd/system"
mkdir -p "${BUILD_DIR}/lib64"
mkdir -p "${BUILD_DIR}/usr/lib"
mkdir -p "${BUILD_DIR}/etc"
mkdir -p "${BUILD_DIR}/home"
mkdir -p "${BUILD_DIR}/sys"
mkdir -p "${BUILD_DIR}/run"
mkdir -p "${BUILD_DIR}/proc"

# Copy binary and services
cp "${BIN_PATH}" "${BUILD_DIR}/usr/bin/"
cp "packaging/contextd.service" "${BUILD_DIR}/usr/lib/systemd/system/"
cp "packaging/contextd-rgb.service" "${BUILD_DIR}/usr/lib/systemd/system/"

# BUNDLE DEPENDENCIES
# This is the "Hardened Portable" strategy: carry your own libs.
echo "Bundling shared library dependencies..."
LIBS=$(ldd "${BIN_PATH}" | grep "=>" | awk '{print $3}' | grep "^/")
for LIB in $LIBS; do
    cp -L "$LIB" "${BUILD_DIR}/usr/lib/"
done

# Copy the loader (ld-linux)
LOADER=$(ldd "${BIN_PATH}" | grep "ld-linux" | awk '{print $1}')
if [ -z "$LOADER" ]; then
    # Fallback for some ldd outputs
    LOADER=$(ldd "${BIN_PATH}" | grep "=>" | grep "ld-linux" | awk '{print $3}')
fi
if [ -n "$LOADER" ]; then
    cp -L "$LOADER" "${BUILD_DIR}/usr/lib/"
    # Symlink for standard loader path
    ln -sf "/usr/lib/$(basename $LOADER)" "${BUILD_DIR}/lib64/$(basename $LOADER)"
fi

# Create os-release for identification
cat <<EOF > "${BUILD_DIR}/etc/os-release"
ID=contextd
NAME="Context Daemon Universal Portable"
PRETTY_NAME="Context Daemon Universal Portable"
EOF

echo "Done. Tree is self-contained."
