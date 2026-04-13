#!/bin/bash
# A thin wrapper around varlinkctl to interact with game-linkd

SOCKET="unix:/run/game-linkd/game-linkd.socket"
INTERFACE="io.github.game-linkd"

usage() {
    echo "Usage: $0 [active|list-games|list-devices|monitor]"
    exit 1
}

if [ -z "$1" ]; then
    usage
fi

case "$1" in
    active)
        varlinkctl call "${SOCKET}" "${INTERFACE}.GetActiveGame" '{}'
        ;;
    list-games)
        varlinkctl call "${SOCKET}" "${INTERFACE}.ListInstalledGames" '{}'
        ;;
    list-devices)
        varlinkctl call "${SOCKET}" "${INTERFACE}.ListDevices" '{}'
        ;;
    monitor)
        # Try 'monitor' first (systemd 255+), fallback to 'call --more'
        if varlinkctl monitor --help > /dev/null 2>&1; then
            varlinkctl monitor "${SOCKET}" "${INTERFACE}.Subscribe" '{}'
        else
            varlinkctl call "${SOCKET}" "${INTERFACE}.Subscribe" '{}' --more=yes
        fi
        ;;
    *)
        usage
        ;;
esac
