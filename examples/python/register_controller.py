#!/usr/bin/env python3
"""
Example Python script to register a controller hint with contextd using varlink.
Requires: pip install varlink
"""

import os
import sys
import varlink

def main():
    # Contextd public socket address
    address = "unix:/run/contextd/public/contextd.socket"
    
    try:
        with varlink.Client(address) as client:
            contextd = client.open('com.performativenonsense.contextd')
            
            # Prepare the controller hint
            hint = {
                "name": "Python Controller Example",
                "version": "1.0.0",
                "pid": os.getpid(),
                "description": "An example script showing cooperative management",
                "website": "https://github.com/shanefagan/contextd",
                "capabilities": ["test", "demo"],
                "interested_devices": ["/dev/hidraw0"] # Replace with a real device path
            }
            
            print(f"Registering hint for PID {os.getpid()}...")
            contextd.RegisterController(hint)
            
            print("\nActive Controllers:")
            controllers = contextd.ListControllers()
            for c in controllers:
                print(f"- {c['name']} (PID: {c['pid']}) - {c.get('description', '')}")
            
            print("\nDevices with hints:")
            devices = contextd.ListDevices()
            for d in devices:
                if d.get('controllers'):
                    print(f"- {d['name']} ({d['path']})")
                    for c in d['controllers']:
                        print(f"  * Managed by: {c['name']}")

            input("\nPress Enter to unregister and exit...")
            contextd.UnregisterController(os.getpid())
            
    except varlink.VarlinkError as e:
        print(f"Varlink error: {e}")
    except ConnectionRefusedError:
        print(f"Could not connect to contextd at {address}. Is it running?")

if __name__ == "__main__":
    main()
