# Hardware Controller Hints

`contextd` provides a mechanism for external applications (like Solaar, OpenRGB, or Piper) to signal that they are managing or monitoring specific hardware devices.

## 🎯 Purpose

In the Linux desktop ecosystem, multiple tools often want to manage the same hardware (e.g., setting a DPI profile vs. setting RGB lighting). Without coordination:
1.  Users may be confused about which app is "in charge".
2.  Apps may step on each other's configurations.
3.  Troubleshooting becomes difficult.

`contextd` acts as a **central bulletin board** where apps can register their interest in a device without asserting exclusive ownership.

## 🛠️ Usage

### Registering a Hint
An app registers a `Controller` object via `RegisterController`.
- **PID**: The process that controls the hardware. It must be the caller itself or one of its ancestors owned by the same user (so a script can register its own PID and then call `varlinkctl`). Pass `0` to mean "the calling process"; sandboxed apps (Flatpak) must do this, because their own PID differs from the one contextd sees. The hint disappears automatically when that process exits.
- **Capabilities**: What the app is doing (e.g., "Lighting", "Macros", "DPI").
- **Interested Devices**: Device nodes the app is managing, such as `/dev/hidraw3`. Any node of a device matches (see `Device.paths`).

Calling `RegisterController` again for the same PID updates the hint.

### Removing a Hint
`UnregisterController(pid)` removes a hint. Only the user that owns the hinted process, or root, can do this. Removing a hint that does not exist succeeds.

### Viewing Hints
When calling `ListDevices` or `ListRGBDevices`, each `Device` includes a `controllers` field with every hint that names one of its `paths`.

```json
{
  "name": "G502 HERO Gaming Mouse",
  "path": "/dev/input/event2",
  "paths": ["/dev/input/event2", "/dev/input/event3", "/dev/hidraw3"],
  "controllers": [
    {
      "name": "Solaar",
      "capabilities": ["DPI", "Battery"],
      "pid": 1234,
      "interested_devices": ["/dev/hidraw3"]
    }
  ]
}
```

## 🔒 Security
Hints are public: anyone can list them. They need no whitelist, but contextd checks the caller's credentials (`SO_PEERCRED`) so one user cannot register hints for, overwrite, or remove another user's processes. Field lengths and the number of hints per user are capped. Errors:

| Error | Meaning |
| :--- | :--- |
| `InvalidController(reason)` | PID is not the caller or an ancestor, the process has exited, or a field is too long |
| `NotOwner(pid)` | The hint belongs to another user |
| `LimitExceeded(limit, max)` | Too many hints for this user, or in total |
