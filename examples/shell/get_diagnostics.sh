#!/bin/bash
# This script uses the varlinkctl utility to query the contextd daemon.

SOCKET="unix:/run/contextd/public/contextd.socket"
INTERFACE="com.performativenonsense.contextd"

if ! command -v varlinkctl &> /dev/null; then
    echo "Error: varlinkctl is not installed. Please install it (usually available in the 'varlink' package)."
    exit 1
fi

echo "Querying system diagnostics from contextd..."
echo "------------------------------------------"

# Use jq if available for pretty printing, otherwise just output the raw JSON
if command -v jq &> /dev/null; then
    varlinkctl call "$SOCKET/$INTERFACE.GetDiagnostics" | jq .
else
    varlinkctl call "$SOCKET/$INTERFACE.GetDiagnostics"
fi
