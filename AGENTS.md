# AI Agents & `contextd`

This document provides guidance for AI Agents (LLMs, Automations, and Copilots) on how to interact with the Context Daemon (`contextd`) to understand the running system state.

## 🤖 For AI Copilots / Developers

If you are an AI assistant working on this repository or a project that consumes its data, here is what you need to know:

### 1. The Context Model
`contextd` is primarily a **read-only context provider**. It does not perform actions or change system state by default. It answers the question: *"What is the user doing right now and what hardware are they using?"* and mostly is focused on gaming use cases, configuration of gaming peripherals like profile switching based on game configs and ensuring for example the hardware is ready to access for user level applications.

> [!NOTE]
> There is an **experimental and optional** RGB lighting interface (`com.performativenonsense.contextd.rgb`). It stores and relays a shared "lighting vibe"; it does not drive hardware itself.

- **Unprivileged reads**: Anyone can query the core socket and the RGB observer socket.
- **Restricted lighting writes**: The RGB control socket is mode `0660`, group `contextd-rgb`. Only root and members of that group can connect. An administrator can narrow this further to specific **system** services with `auth.authorized_units` in `/etc/contextd/config.toml` (user-session units never match, because users can name their own units anything).
- **Self-attested controller hints**: `RegisterController()` needs no whitelist, but the hint's `pid` must be the caller or one of its ancestors owned by the same user (pass `0` for "this process", which sandboxed apps must do). Only that user, or root, can remove the hint.
- **Focus reports**: `ReportFocus()` is for session helpers such as `contextd-focus`. The reported process must belong to the caller's user, and a report lasts only while the reporting process runs.
- **Data integrity**: Game PIDs come from `/proc`; hint and focus PIDs are checked against the caller's credentials (`SO_PEERCRED`) and dropped when the process exits.

### 2. Available Interfaces
The daemon exposes its state via **Varlink** on three separate Unix sockets:
- `/run/contextd/public/contextd.socket` (Core system & game context, **0666**)
- `/run/contextd/public/contextd-rgb-observer.socket` (Public RGBA vibe, **0666**)
- `/run/contextd/private/contextd-rgb-control.socket` (RGBA control, **0660**, group `contextd-rgb`)

| Socket | Interface | Access | Purpose |
| :--- | :--- | :--- | :--- |
| `contextd.socket` | `com.performativenonsense.contextd` | Everyone | System and game discovery, controller hints |
| `contextd-rgb-observer.socket` | `com.performativenonsense.contextd.rgb.Observer` | Everyone | Read/subscribe to the lighting vibe |
| `contextd-rgb-control.socket` | `com.performativenonsense.contextd.rgb.Control` | root + `contextd-rgb` | Set the lighting vibe |

The interface definitions live in `contextd-client/interfaces/*.varlink`; they are the source of truth for both the daemon and the Rust client.

Key methods to call:
- `com.performativenonsense.contextd.GetActiveGame()`: The game whose window has focus, when the `contextd-focus` session helper is running; otherwise the most recently launched running game. `Game.focused` is `true`/`false` when focus is known and `null` when it is not.
- `com.performativenonsense.contextd.ListRunningGames()`: Every running game, focused one first, then newest first.
- `com.performativenonsense.contextd.SubscribeRunningGames()` / `SubscribeDevices()`: The same lists, streamed on every change (call with `more: true`). Prefer these to polling.
- `com.performativenonsense.contextd.ListRGBDevices()`: Devices with RGB lighting, as listed in OpenRGB's installed udev rules (or tagged `ID_CONTEXTD_RGB=1`). These also carry the class `rgb` in `ListDevices()`.
- `com.performativenonsense.contextd.rgb.Observer.SubscribeLightingContext()`: Subscribe to real-time RGBA vibe updates (call with `more: true`). Pass `packed: true` to get the matrix as base64 RGBA (`packed_matrix`).
- `com.performativenonsense.contextd.rgb.Control.SetLightingContext()`: Update the system-wide lighting context. Send matrices as `packed_matrix` (base64 RGBA, about 6× smaller) or `matrix`; pass `clear_matrix: true` to remove the matrix.

### 3. Integration Philosophy
- **Subscribe rather than poll**: `SubscribeRunningGames` notices new games within about a second and focus changes immediately; `SubscribeDevices` reacts to udev changes as they happen. Polling still works: results are cached (running games 5 s, installed games 60 s, hardware 10 s, diagnostics 5 min by default).
- **Keep connections few**: Each socket allows 16 concurrent connections per user by default, and idle connections are closed after 2 minutes. Reuse one connection rather than opening many.
- **Only update when asked**: Do not update the daemon unless explicitly asked to do so.

## 🛰️ Use Cases for Agents

AI Agents can leverage `contextd` to:
- **Dynamic Profile Switching**: An agent can subscribe to `SubscribeRunningGames()` and adjust system profiles, fan speeds, or lighting when the focused game changes.
- **Hardware Debugging**: If a user asks "Why isn't my mouse working?", an agent can check `ListDevices()` to see if the hardware is detected and if permissions (`uaccess`) are correct.
- **System Sanity Checks**: Use `GetDiagnostics()` to verify if a user's system meets specific game requirements (VRAM, RAM) or to identify if they are missing critical graphics drivers/libraries (Vulkan/OpenGL). `vram_total` is 0 when the driver does not report it (only amdgpu does).
- **Game Stats Integration**: Use the detected AppID to fetch external game metadata or launch specific companion overlays.
- **Cooperative Hardware Management**: Use `RegisterController()` to signal that your agent is managing specific hardware (e.g., "AI Macro Engine"). Other apps like Solaar or OpenRGB will see this hint and can avoid conflicting configurations or suggest troubleshooting steps to the user.
