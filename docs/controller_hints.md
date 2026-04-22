# Hardware Controller Hints

`contextd` provides a public endpoint for device controller software (e.g., RGB controllers, DPI managers, macro engines) to register their "interest" in specific hardware devices.

This is a **cooperative, non-authoritative** system. It does not lock devices or prevent other apps from accessing them. Instead, it serves as a discovery mechanism so that different applications can be aware of each other and help the user resolve potential conflicts.

## How it Works

1.  **Registration**: An application calls `RegisterController()` providing its PID, capabilities, and a list of device paths it is interested in.
2.  **Discovery**: Other applications calling `ListDevices()` or `ListRGBDevices()` will see an embedded list of "interested" controllers for each device.
3.  **Lifecycle**: `contextd` monitors the PIDs of registered controllers. If a process terminates, its hint is automatically removed from the system.

## Varlink Interface

### `Controller` Type

| Field | Type | Description |
| :--- | :--- | :--- |
| `name` | `string` | Human-readable name (e.g., "OpenRGB") |
| `version` | `?string` | Optional version string |
| `pid` | `int` | Process ID of the controller |
| `description` | `?string` | Short description of what the app is doing |
| `website` | `?string` | Link to documentation or help |
| `capabilities` | `[]string` | List of features (e.g., `["rgb", "dpi", "battery"]`) |
| `interested_devices`| `[]string` | List of device paths (e.g., `["/dev/hidraw0"]`) |

### Methods

*   `RegisterController(controller: Controller)`: Register or update a hint.
*   `UnregisterController(pid: int)`: Manually remove a hint.
*   `ListControllers()`: List all active hints.

## Example Use Cases

### Troubleshooting Conflicts
If Solaar is running and managing a Logitech mouse, and a user opens another configuration tool that fails to apply settings, that tool can check `ListDevices()` and see:
> "Hey, Solaar (PID 1234) is already interested in this device. You might need to close it or adjust its settings."

### Cooperative RGB
Multiple RGB applications can signal which devices they are "watching". While `contextd` doesn't enforce exclusive access, it provides the metadata needed for apps to "play nice" or for the user to understand why their lighting is flickering (due to multiple managers).

## Testing with `contextctl`

You can register a temporary hint for your current shell using the provided helper script:

```bash
./scripts/contextctl.sh hint "MyAgent" "/dev/hidraw0"
```

This will register the hint and keep it active until you press `Ctrl+C`.
