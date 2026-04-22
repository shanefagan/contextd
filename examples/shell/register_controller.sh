#!/bin/bash
# Example script to register a controller hint with contextd

# Get current PID
PID=$$

# Register Solaar-like hint
varlink call unix:/run/contextd/public/contextd.socket/com.performativenonsense.contextd.RegisterController '{
  "controller": {
    "name": "Solaar",
    "version": "1.1.13",
    "pid": '$PID',
    "description": "Logitech device manager",
    "website": "https://pwr-solaar.github.io/Solaar/",
    "capabilities": ["battery", "input", "rgb"],
    "interested_devices": ["/dev/hidraw0", "/dev/hidraw1"]
  }
}'

echo "Registered Solaar hint for PID $PID"
echo "Press Enter to list ALL CONTROLLERS..."
read
varlink call unix:/run/contextd/public/contextd.socket/com.performativenonsense.contextd.ListControllers

echo "Press Enter to list ALL DEVICES (will show Solaar hint next to /dev/hidraw0)..."
read
varlink call unix:/run/contextd/public/contextd.socket/com.performativenonsense.contextd.ListDevices

echo "Press Enter to unregister and exit..."
read

varlink call unix:/run/contextd/public/contextd.socket/com.performativenonsense.contextd.UnregisterController '{"pid": '$PID'}'
