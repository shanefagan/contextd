#!/bin/bash
# Simple CLI wrapper for contextd using varlinkctl

ADDR="unix:/run/contextd/contextd.socket"
RGB_OBS_ADDR="unix:/run/contextd/contextd-rgb-observer.socket"
RGB_CTRL_ADDR="unix:/run/contextd/contextd-rgb-control.socket"

usage() {
    echo "Usage: $0 [active|list-games|list-devices|list-rgb|diagnostics|rgb-get|rgb-set|rgb-subscribe]"
    echo "  rgb-get/subscribe use the PUBLIC observer socket (0666)"
    echo "  rgb-set uses the PRIVATE control socket (0660, contextd-rgb group)"
    echo "  rgb-set R G B [A]  (Alpha defaults to 255)"
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
    diagnostics)
        varlinkctl call $ADDR com.performativenonsense.contextd.GetDiagnostics "{}"
        ;;
    rgb-get)
        varlinkctl call $RGB_OBS_ADDR com.performativenonsense.contextd.rgb.Observer.GetLightingContext "{}"
        ;;
    rgb-subscribe)
        varlinkctl call --more $RGB_OBS_ADDR com.performativenonsense.contextd.rgb.Observer.SubscribeLightingContext "{}"
        ;;
    rgb-set)
        # Usage: rgb-set R G B [A]
        R=$1; G=$2; B=$3; A=${4:-255}
        varlinkctl call $RGB_CTRL_ADDR com.performativenonsense.contextd.rgb.Control.SetLightingContext "{\"main_color\": {\"r\": $R, \"g\": $G, \"b\": $B, \"a\": $A}}"
        ;;
    *)
        usage
        ;;
esac
