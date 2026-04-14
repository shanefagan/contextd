# AI Agents & `contextd`

This document provides guidance for AI Agents (LLMs, Automations, and Copilots) on how to interact with the Context Daemon (`contextd`) to understand the running system state.

## 🤖 For AI Copilots / Developers

If you are an AI assistant working on this repository or a project that consumes its data, here is what you need to know:

### 1. The Context Model
`contextd` is a **read-only context provider**. It does not perform actions or change system state. It answers the question: *"What is the user doing right now and what hardware are they using?"* and mostly is focused on gaming use cases, configuration of gaming peripherals like profile switching based on game configs and ensuring for example the hardware is ready to access for user level applications.

### 2. Available Interfaces
The daemon exposes its state via **Varlink** on the Unix socket `/run/contextd/contextd.socket`.

Key methods to call:
- `com.performativenonsense.contextd.Active()`: Returns the currently foregrounded game or high-performance app.
- `com.performativenonsense.contextd.ListDevices()`: Returns a list of connected interaction hardware (Mice, Keyboards, Controllers).
- `com.performativenonsense.contextd.ListRgb()`: Returns a list of addressable lighting/aesthetic hardware.
- `com.performativenonsense.contextd.GetDiagnostics()`: Returns system sanity information (RAM, GPU, Vulkan/OpenGL support, and environment info).

### 3. Integration Philosophy
- **Polling is Expected**: The daemon uses a internal 10-second cache. You can poll frequently without impacting system performance.
- **Unprivileged Access**: No root/sudo is required to query the socket.
- **Data Integrity**: Process IDs and hardware nodes are verified by the daemon before being reported.
- **Only update when asked**: Do not update the daemon unless explicitly asked to do so.

## 🛰️ Use Cases for Agents

AI Agents can leverage `contextd` to:
- **Dynamic Profile Switching**: An agent can monitor `Active()` and automatically adjust system profiles, fan speeds, or lighting based on the detected app.
- **Hardware Debugging**: If a user asks "Why isn't my mouse working?", an agent can check `ListDevices()` to see if the hardware is detected and if permissions (`uaccess`) are correct.
- **System Sanity Checks**: Use `GetDiagnostics()` to verify if a user's system meets specific game requirements (VRAM, RAM) or to identify if they are missing critical graphics drivers/libraries (Vulkan/OpenGL).
- **Game Stats Integration**: Use the detected AppID to fetch external game metadata or launch specific companion overlays.
