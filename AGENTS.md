# AI Agents & `contextd`

This document provides guidance for AI Agents (LLMs, Automations, and Copilots) on how to interact with the Context Daemon (`contextd`) to understand the running system state.

## 🤖 For AI Copilots / Developers

If you are an AI assistant working on this repository or a project that consumes its data, here is what you need to know:

### 1. The Context Model
`contextd` is primarily a **read-only context provider**. It does not perform actions or change system state by default. It answers the question: *"What is the user doing right now and what hardware are they using?"* and mostly is focused on gaming use cases, configuration of gaming peripherals like profile switching based on game configs and ensuring for example the hardware is ready to access for user level applications.

> [!NOTE]
> There is an **experimental and optional** RGB control interface available (`com.performativenonsense.contextd.rgb`) which allows for active state changes on supported hardware when enabled.

- **Unprivileged Access**: No root/sudo is required to query the core socket or RGB sockets.
- **Authoritative Control**: To authorize an application to **change system state** (e.g., an RGB controller), a superuser must add the application's systemd unit name (e.g., `openrgb.service`) to the `authorized_units` list in `/etc/contextd/config.toml`. Informational methods like `RegisterController()` do not require this.
- **Data Integrity**: Process IDs and hardware nodes are verified by the daemon before being reported.

### 2. Available Interfaces
The daemon exposes its state via **Varlink** on three separate Unix sockets:
- `/run/contextd/public/contextd.socket` (Core system & game context)
- `/run/contextd/public/contextd-rgb-observer.socket` (Public RGBA vibe - **0666**)
- `/run/contextd/private/contextd-rgb-control.socket` (Private RGBA control - **0666**)

| Socket | Interface | Access | Purpose |
| :--- | :--- | :--- | :--- |
| `contextd.socket` | `com.performativenonsense.contextd` | Public | System and Game Discovery |
| `contextd-rgb-observer.socket` | `com.performativenonsense.contextd.rgb.Observer` | Public | Consume/Subscribe to lighting vibe |
| `contextd-rgb-control.socket` | `com.performativenonsense.contextd.rgb.Control` | Private | Set/Update system lighting |

Key methods to call:
- `com.performativenonsense.contextd.GetActiveGame()`: Returns the currently foregrounded game.
- `com.performativenonsense.contextd.rgb.Observer.SubscribeLightingContext()`: Subscribe to real-time RGBA vibe updates.
- `com.performativenonsense.contextd.rgb.Control.SetLightingContext()`: Update the system-wide lighting context.

### 3. Integration Philosophy
- **Polling is Expected**: The daemon uses a internal 10-second cache. You can poll frequently without impacting system performance.
- **Unprivileged Access**: No root/sudo is required to query the core socket or RGB sockets.
- **Data Integrity**: Process IDs and hardware nodes are verified by the daemon before being reported.
- **Only update when asked**: Do not update the daemon unless explicitly asked to do so.

## 🛰️ Use Cases for Agents

AI Agents can leverage `contextd` to:
- **Dynamic Profile Switching**: An agent can monitor `Active()` and automatically adjust system profiles, fan speeds, or lighting based on the detected app.
- **Hardware Debugging**: If a user asks "Why isn't my mouse working?", an agent can check `ListDevices()` to see if the hardware is detected and if permissions (`uaccess`) are correct.
- **System Sanity Checks**: Use `GetDiagnostics()` to verify if a user's system meets specific game requirements (VRAM, RAM) or to identify if they are missing critical graphics drivers/libraries (Vulkan/OpenGL).
- **Game Stats Integration**: Use the detected AppID to fetch external game metadata or launch specific companion overlays.
- **Cooperative Hardware Management**: Use `RegisterController()` to signal that your agent is managing specific hardware (e.g., "AI Macro Engine"). Other apps like Solaar or OpenRGB will see this hint and can avoid conflicting configurations or suggest troubleshooting steps to the user.
