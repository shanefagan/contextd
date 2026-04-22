#!/usr/bin/env python3
import socket
import json
import sys

# Path to the public RGB observer socket
SOCKET_PATH = "/run/contextd/public/contextd-rgb-observer.socket"

def main():
    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.connect(SOCKET_PATH)
    except FileNotFoundError:
        print(f"Error: Could not connect to {SOCKET_PATH}. Is contextd running in RGB mode?")
        sys.exit(1)

    # Send a subscription request with "more": True
    request = {
        "method": "com.performativenonsense.contextd.rgb.Observer.SubscribeLightingContext",
        "parameters": {},
        "more": True
    }
    
    message = json.dumps(request).encode('utf-8') + b'\0'
    sock.sendall(message)

    print("Subscribed to RGB lighting updates. Press Ctrl+C to stop.")

    # Read the stream of responses
    buffer = bytearray()
    try:
        while True:
            chunk = sock.recv(4096)
            if not chunk:
                print("Connection closed by daemon.")
                break
                
            buffer.extend(chunk)
            
            # Process complete messages (separated by null bytes)
            while b'\0' in buffer:
                msg_bytes, buffer = buffer.split(b'\0', 1)
                
                try:
                    response = json.loads(msg_bytes.decode('utf-8'))
                    if "error" in response:
                        print(f"Error: {response['error']}")
                        continue
                        
                    params = response.get("parameters", {})
                    color = params.get("main_color")
                    
                    if color:
                        print(f"New Vibe Color -> R: {color.get('r'):3} | G: {color.get('g'):3} | B: {color.get('b'):3} | A: {color.get('a'):3}")
                    
                except json.JSONDecodeError:
                    pass

    except KeyboardInterrupt:
        print("\nUnsubscribing...")
    finally:
        sock.close()

if __name__ == "__main__":
    main()
