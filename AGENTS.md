# AI Agents & `contextd`

This document provides guidance for AI Agents (LLMs, Automations, and Copilots) on how to interact with the Context Daemon (`contextd`) to understand the running system state.

## 🤖 For AI Copilots / Developers

If you are an AI assistant working on this repository or a project that consumes its data, here is what you need to know:

### 1. The Context Model
`contextd` is a **read-only context provider**. It does not perform actions or change system state. It answers the question: *"What is the user doing right now and what hardware are they using?"*

### 2. Available Interfaces
The daemon exposes its state via **Varlink** on the Unix socket `/run/contextd/contextd.socket`.

Key methods to call:
- `com.performativenonsense.contextd.Active()`: Returns the currently foregrounded game or high-performance app.
- `com.performativenonsense.contextd.ListDevices()`: Returns a list of connected interaction hardware (Mice, Keyboards, Controllers).
- `com.performativenonsense.contextd.ListRgb()`: Returns a list of addressable lighting/aesthetic hardware.

### 3. Integration Philosophy
- **Polling is Expected**: The daemon uses a internal 10-second cache. You can poll frequently without impacting system performance.
- **Unprivileged Access**: No root/sudo is required to query the socket.
- **Data Integrity**: Process IDs and hardware nodes are verified by the daemon before being reported.

## 🛰️ Use Cases for Agents

AI Agents can leverage `contextd` to:
- **Dynamic Profile Switching**: An agent can monitor `Active()` and automatically adjust system profiles, fan speeds, or lighting based on the detected app.
- **Hardware Debugging**: If a user asks "Why isn't my mouse working?", an agent can check `ListDevices()` to see if the hardware is detected and if permissions (`uaccess`) are correct.
- **Game Stats Integration**: Use the detected AppID to fetch external game metadata or launch specific companion overlays.
