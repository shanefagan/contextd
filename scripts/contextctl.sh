#!/bin/bash
# Simple CLI wrapper for contextd using varlinkctl

ADDR="unix:/run/contextd/contextd.socket"

usage() {
    echo "Usage: $0 [active|list-games|list-devices|list-rgb]"
    exit 1
}

if [ $# -lt 1 ]; then
    usage
fi

CMD=$1
shift

case $CMD in
    active)
        varlinkctl call $ADDR com.performativenonsense.contextd.GetActiveGame "{}"
        ;;
    list-games)
        varlinkctl call $ADDR com.performativenonsense.contextd.ListInstalledGames "{}"
        ;;
    list-devices)
        varlinkctl call $ADDR com.performativenonsense.contextd.ListDevices "{}"
        ;;
    list-rgb)
        varlinkctl call $ADDR com.performativenonsense.contextd.ListRGBDevices "{}"
        ;;
    *)
        usage
        ;;
esac
