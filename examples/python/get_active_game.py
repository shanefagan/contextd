#!/usr/bin/env python3
import socket
import json
import sys

# Path to the public core socket
SOCKET_PATH = "/run/contextd/public/contextd.socket"

def main():
    try:
        # Create a Unix domain socket
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.connect(SOCKET_PATH)
    except FileNotFoundError:
        print(f"Error: Could not connect to {SOCKET_PATH}. Is contextd running?")
        sys.exit(1)

    # Varlink requests are JSON objects ending with a null byte
    request = {
        "method": "com.performativenonsense.contextd.GetActiveGame",
        "parameters": {}
    }
    
    # Send the request
    message = json.dumps(request).encode('utf-8') + b'\0'
    sock.sendall(message)

    # Read the response (read until the null byte)
    data = bytearray()
    while True:
        chunk = sock.recv(1024)
        if not chunk:
            break
        data.extend(chunk)
        if b'\0' in chunk:
            break

    # Parse and print the response
    response_str = data.split(b'\0')[0].decode('utf-8')
    response = json.loads(response_str)

    if "error" in response:
        print(f"Error: {response['error']}")
    else:
        game = response.get("parameters", {}).get("game")
        if game:
            print(f"Active Game Detected:")
            print(f"  Name:   {game.get('name')}")
            print(f"  Source: {game.get('source')}")
            print(f"  PID:    {game.get('pid')}")
        else:
            print("No active game is currently foregrounded.")

if __name__ == "__main__":
    main()
