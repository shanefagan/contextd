import sys
import json
import socket
import time
import math

def main():
    # Provide default values if not specified
    width = 8
    height = 8
    effect = "wave"
    frames = 150

    if len(sys.argv) > 1:
        if sys.argv[1] in ["-h", "--help", "help"]:
            print("Usage: python animate_matrix.py [width] [height] [effect] [frames]")
            print("Usage: python animate_matrix.py [size] [effect] [frames] (assumes square)")
            print("Available effects: wave, pulse, checker, sweep, noise")
            sys.exit(0)
        
        # Determine if arg 2 is a number (height) or a string (effect)
        width = int(sys.argv[1])
        if len(sys.argv) > 2:
            try:
                height = int(sys.argv[2])
                offset = 1
            except ValueError:
                height = width
                offset = 0
                
            if len(sys.argv) > 2 + offset:
                effect = sys.argv[2 + offset]
            if len(sys.argv) > 3 + offset:
                frames = int(sys.argv[3 + offset])

    print(f"Animating an {width}x{height} RGB matrix with effect '{effect}' for {frames} frames...")
    
    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.connect("/run/contextd/contextd-rgb-control.socket")
    except Exception as e:
        print(f"Failed to connect to control socket: {e}")
        sys.exit(1)

    total = width * height

    for f in range(frames):
        # Base ambient color
        mr = (10 + f * 2) % 60
        mg = (10 + f * 1) % 40
        mb = (30 + f * 3) % 80
        
        matrix_data = []
        for i in range(total):
            x = i % width
            y = i // width
            
            if effect == "wave":
                r = int(x * 255 / width + f * 10) % 255
                g = int(y * 255 / height + f * 8) % 255
                b = int(255 - ((x + y) * 255 / (width + height) + f * 15)) % 255
            elif effect == "pulse":
                intensity = int((math.sin(f * 0.1) + 1) * 127)
                r = intensity
                g = 0
                b = 255 - intensity
            elif effect == "checker":
                if (x + y + (f // 4)) % 2 == 0:
                    r = (f * 15) % 255
                    g = 255 - ((f * 10) % 255)
                    b = 255
                else:
                    r, g, b = 15, 15, 25
            elif effect == "sweep":
                col = f % width
                dist = abs(x - col)
                val = int(255 - (dist * 255 / width * 1.5))
                val = max(0, min(255, val))
                r = val
                g = int(val * 0.2)
                b = 255 - val
            elif effect == "noise":
                r = ((x * 123) + (y * 321) + (f * 111)) % 255
                g = ((x * 321) + (y * 123) + (f * 222)) % 255
                b = ((x * 222) + (y * 111) + (f * 333)) % 255
            else:
                # Fallback to wave
                r = int(x * 255 / width + f * 10) % 255
                g = int(y * 255 / height + f * 8) % 255
                b = int(255 - ((x + y) * 255 / (width + height) + f * 15)) % 255
                
            matrix_data.append({
                "r": max(0, min(255, r)), 
                "g": max(0, min(255, g)), 
                "b": max(0, min(255, b)), 
                "a": 255
            })
        
        req = {
            "method": "com.performativenonsense.contextd.rgb.Control.SetLightingContext",
            "parameters": {
                "main_color": {"r": mr, "g": mg, "b": mb, "a": 255},
                "matrix": {
                    "width": width,
                    "height": height,
                    "data": matrix_data
                }
            }
        }
        
        # Send json payload
        msg = json.dumps(req).encode('utf-8') + b'\0'
        try:
            sock.sendall(msg)
            sock.settimeout(0.1)
            try:
                resp = sock.recv(4096)
            except socket.timeout:
                pass
            sock.settimeout(None)
        except Exception as e:
            print(f"\nSocket error: {e}")
            break
            
        sys.stdout.write(f"\rFrame {f+1}/{frames} sent [Effect: {effect}]")
        sys.stdout.flush()
        
        # Cap FPS for large matrix sizes to avoid high CPU usage
        target_fps = 20
        if total > 4096:
            target_fps = 2
        elif total > 1024:
            target_fps = 5
        
        time.sleep(1.0 / target_fps)

    print("\nDone!")
    sock.close()

if __name__ == "__main__":
    main()
