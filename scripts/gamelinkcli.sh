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
        # Use -E (short for --more --timeout=infinity) for monitoring events
        varlinkctl call "${SOCKET}" "${INTERFACE}.Subscribe" '{}' -E
        ;;
    *)
        usage
        ;;
esac
