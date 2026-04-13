#!/usr/bin/env python3
import socket
import json
import sys
import argparse

SOCKET_PATH = "/run/contextd/contextd.socket"

def varlink_call(method, params=None):
    if params is None:
        params = {}
    
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.connect(SOCKET_PATH)
            
            # Prepare request
            req = {
                "method": f"io.github.contextd.{method}",
                "parameters": params
            }
            s.sendall(json.dumps(req).encode() + b'\0')
            
            # Read response
            buffer = b""
            while True:
                chunk = s.recv(4096)
                if not chunk:
                    break
                buffer += chunk
                if b'\0' in buffer:
                    break
            
            res = buffer.split(b'\0')[0].decode()
            return json.loads(res)
    except Exception as e:
        return {"error": str(e)}

def monitor():
    print(f"--- Monitoring contextd Events (Ctrl+C to stop) ---")
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.connect(SOCKET_PATH)
            
            req = {
                "method": "io.github.contextd.Subscribe",
                "parameters": {},
                "more": True
            }
            s.sendall(json.dumps(req).encode() + b'\0')
            
            buffer = b""
            while True:
                chunk = s.recv(4096)
                if not chunk:
                    break
                buffer += chunk
                
                while b'\0' in buffer:
                    line, buffer = buffer.split(b'\0', 1)
                    event = json.loads(line.decode())
                    
                    if "parameters" in event:
                        p = event["parameters"]
                        etype = p.get("event_type", "unknown")
                        print(f"🔔 EVENT: {etype}")
                        if p.get("game"):
                            print(f"   🎮 Game: {p['game']['name']} ({p['game']['source']})")
                        if p.get("device"):
                            print(f"   🔌 Device: {p['device']['name']} [{p['device']['classes'][0]}]")
                        print("-" * 30)
    except KeyboardInterrupt:
        print("\nMonitor stopped.")
    except Exception as e:
        print(f"Error: {e}")

fn_header = lambda t: print(f"\n\033[1;34m=== {t} ===\033[0m")

def dashboard():
    fn_header("ACTIVE SESSION")
    active = varlink_call("GetActiveGame")
    if active.get("parameters", {}).get("game"):
        g = active["parameters"]["game"]
        print(f"📌 {g['name']} (PID: {g['pid']}, via {g['source']})")
    else:
        print("   No active game detected.")

    fn_header("GAMING HARDWARE")
    hw = varlink_call("ListDevices")
    devices = hw.get("parameters", {}).get("devices", [])
    for d in devices:
        uaccess = "✅" if d['has_uaccess'] else "❌"
        print(f"   {uaccess} {d['name']} ({d['vendor']}) - Classes: {', '.join(d['classes'])}")

    fn_header("RGB & SYSTEM")
    rgb = varlink_call("ListRGBDevices")
    devices = rgb.get("parameters", {}).get("devices", [])
    for d in devices:
        print(f"   💡 {d['name']} ({d['vendor']}) - Path: {d['path']}")

    fn_header("INSTALLED GAMES (Quick List)")
    games = varlink_call("ListInstalledGames")
    glist = games.get("parameters", {}).get("games", [])
    sources = {}
    for g in glist:
        sources[g['source']] = sources.get(g['source'], 0) + 1
    
    for s, count in sources.items():
        print(f"   📦 {s}: {count} games")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="contextd Debugging Tool")
    parser.add_argument("mode", choices=["dash", "monitor"], help="dash: system summary, monitor: live events")
    
    args = parser.parse_args()
    
    if args.mode == "dash":
        dashboard()
    else:
        monitor()
