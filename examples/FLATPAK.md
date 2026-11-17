# Flatpak Integration with `contextd`

Applications running inside a Flatpak sandbox can still interact with `contextd`, but they must be granted permission to access the Unix domain sockets exposed by the daemon.

## Granting Access

To allow a Flatpak application to communicate with `contextd`, you need to expose the directory containing the sockets.

### 1. For the Public Sockets (Core & RGB Observer)
The public sockets live in `/run/contextd/public`. Most applications (like OBS or game overlays) only need read access to this directory to query the active game or observe lighting vibes.

**Via Command Line:**
```bash
flatpak override <app-id> --filesystem=/run/contextd/public:ro
```

**In a Flatpak Manifest (JSON):**
```json
"finish-args": [
    "--filesystem=/run/contextd/public:ro"
]
```

### 2. For the Private Socket (RGB Control)
The RGB Control socket lives in `/run/contextd/private`. It is mode 0660, group `contextd-rgb`: the filesystem override only makes it visible, and the user running the app must also be in that group.

**Via Command Line:**
```bash
flatpak override <app-id> --filesystem=/run/contextd/private:rw
```

**In a Flatpak Manifest (JSON):**
```json
"finish-args": [
    "--filesystem=/run/contextd/private:rw"
]
```

## Security Note

Since `contextd` uses Varlink over Unix sockets, the security is handled at the filesystem level. By granting access to the `/run/contextd` subdirectories, the Flatpak application is able to see and connect to the sockets just like a native host application.

The daemon itself uses file permissions on the sockets (`0666` for the public ones, `0660` with group `contextd-rgb` for control) to further manage access, but the Flatpak sandbox requires the explicit filesystem mount to even see the socket nodes.

## Controller hints from a sandbox

A sandboxed app sees its own PID namespace, so its PID is not the one contextd sees. Register hints with `"pid": 0`, which tells contextd to use the caller's real PID.
