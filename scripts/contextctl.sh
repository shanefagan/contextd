#!/bin/bash
# Simple CLI wrapper for contextd using varlinkctl

ADDR="unix:/run/contextd/contextd.socket"

usage() {
    echo "Usage: $0 [active|list-games|list-devices|list-rgb|monitor]"
    exit 1
}

if [ $# -lt 1 ]; then
    usage
fi

CMD=$1
shift

case $CMD in
    active)
        varlinkctl call $ADDR io.github.contextd.GetActiveGame "{}"
        ;;
    list-games)
        varlinkctl call $ADDR io.github.contextd.ListInstalledGames "{}"
        ;;
    list-devices)
        varlinkctl call $ADDR io.github.contextd.ListDevices "{}"
        ;;
    list-rgb)
        varlinkctl call $ADDR io.github.contextd.ListRGBDevices "{}"
        ;;
    monitor)
        # Use -E for 'more' and infinity timeout
        varlinkctl call $ADDR io.github.contextd.Subscribe "{}" -E
        ;;
    *)
        usage
        ;;
esac
