#!/bin/bash
# Simple CLI wrapper for contextd using varlinkctl

ADDR="unix:/run/contextd/public/contextd.socket"
RGB_OBS_ADDR="unix:/run/contextd/public/contextd-rgb-observer.socket"
RGB_CTRL_ADDR="unix:/run/contextd/private/contextd-rgb-control.socket"

usage() {
    echo "Usage: $0 [active|running|watch-games|watch-devices|list-games|list-devices|list-rgb|list-controllers|hint|diagnostics|rgb-get|rgb-set|rgb-set-matrix|rgb-clear-matrix|rgb-subscribe]"
    echo "  active                Most recently launched running game"
    echo "  running               All running games"
    echo "  watch-games           Stream running games as they change"
    echo "  watch-devices         Stream the device list as it changes"
    echo "  list-controllers      List apps that have registered interest in devices"
    echo "  hint NAME DEVICE...   Register a quick hint for the current shell"
    echo "  rgb-get/subscribe use the PUBLIC observer socket (0666)"
    echo "  rgb-set* use the PRIVATE control socket (0660, contextd-rgb group)"
    echo "  rgb-set R G B [A]  (Alpha defaults to 255)"
    echo "  rgb-set-matrix W H R G B [A]  (Sets an entire WxH matrix to a solid color)"
    echo "  rgb-clear-matrix   Removes the matrix, keeping the main color"
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
    running)
        varlinkctl call $ADDR com.performativenonsense.contextd.ListRunningGames "{}"
        ;;
    watch-games)
        varlinkctl call --more $ADDR com.performativenonsense.contextd.SubscribeRunningGames "{}"
        ;;
    watch-devices)
        varlinkctl call --more $ADDR com.performativenonsense.contextd.SubscribeDevices "{}"
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
    list-controllers)
        varlinkctl call $ADDR com.performativenonsense.contextd.ListControllers "{}"
        ;;
    hint)
        NAME=$1; shift
        DEVICES=""
        for dev in "$@"; do
            if [ -n "$DEVICES" ]; then DEVICES="$DEVICES, "; fi
            DEVICES="$DEVICES\"$dev\""
        done
        varlinkctl call $ADDR com.performativenonsense.contextd.RegisterController "{
            \"controller\": {
                \"name\": \"$NAME\",
                \"pid\": $$,
                \"description\": \"Manual hint from contextctl\",
                \"capabilities\": [\"manual\"],
                \"interested_devices\": [$DEVICES]
            }
        }"
        # varlinkctl is our child, so the daemon accepts $$ (its parent) as the hint PID.
        echo "Hint registered for PID $$. Press Ctrl+C to unregister and exit."
        # Keep alive until interrupted
        trap "varlinkctl call $ADDR com.performativenonsense.contextd.UnregisterController '{\"pid\": $$}'; exit" SIGINT SIGTERM
        while true; do sleep 1; done
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
    rgb-set-matrix)
        # Usage: rgb-set-matrix W H R G B [A]
        W=$1; H=$2; R=$3; G=$4; B=$5; A=${6:-255}
        # Use awk to generate the payload fast
        JSON=$(awk -v w="$W" -v h="$H" -v r="$R" -v g="$G" -v b="$B" -v a="$A" '
        BEGIN {
            total = w * h;
            printf "{\"matrix\": {\"width\": %d, \"height\": %d, \"data\": [", w, h;
            for (i=0; i<total; i++) {
                printf "{\"r\": %d, \"g\": %d, \"b\": %d, \"a\": %d}", r, g, b, a;
                if (i < total - 1) printf ", ";
            }
            printf "]}}";
            exit;
        }')
        varlinkctl call $RGB_CTRL_ADDR com.performativenonsense.contextd.rgb.Control.SetLightingContext "$JSON"
        ;;
    rgb-clear-matrix)
        varlinkctl call $RGB_CTRL_ADDR com.performativenonsense.contextd.rgb.Control.SetLightingContext '{"clear_matrix": true}'
        ;;
    *)
        usage
        ;;
esac
