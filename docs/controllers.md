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
- **PID**: The process ID of the controller app (used for auto-pruning if the app crashes).
- **Capabilities**: What the app is doing (e.g., "Lighting", "Macros", "DPI").
- **Interested Devices**: A list of udev paths the app is managing.

### Viewing Hints
When calling `ListDevices` or `ListRGBDevices`, the returned `Device` objects include a `controllers` field containing all registered hints for that specific path.

```json
{
  "name": "Logitech G502",
  "path": "/dev/input/event2",
  "controllers": [
    {
      "name": "Solaar",
      "capabilities": ["DPI", "Battery"],
      "pid": 1234
    }
  ]
}
```

## 🔒 Security
Since this method exposes which processes are running and what they are doing, `RegisterController` is a **restricted method**. The calling process must have its systemd unit whitelisted in `config.toml`.
