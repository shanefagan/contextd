# System Architecture

This document explains the internal design of `contextd` and how external applications interact with the system.

## 🏗️ High-Level Structure

`contextd` follows a "dumb daemon, smart client" philosophy. It acts as a central hub that collects system state and exposes it via a typed interface.

```mermaid
classDiagram
    class Main {
        +run_server()
        +spawn_permission_fixer()
    }

    class ContextService {
        +get_active_game()
        +list_devices()
        +register_controller()
    }

    class RgbService {
        +set_lighting_context()
        +subscribe_lighting_context()
    }

    class GameManager {
        -detectors: Vec~GameDetector~
        +get_active_game()
    }

    class HardwareManager {
        -detectors: Vec~HardwareDetector~
        +list_all_devices()
    }

    class Config {
        +Lazy CONFIG
        +load()
    }

    class Auth {
        +verify_unit_access()
        +get_current_peer()
    }

    Main --> ContextService
    Main --> RgbService
    ContextService --> GameManager
    ContextService --> HardwareManager
    ContextService --> Auth
    RgbService --> Auth
    ContextService --> Config
```

### Key Components:
1.  **Detectors**: Pluggable modules that gather data from various sources:
    *   **Game Detectors**: Poll Steam, Lutris, and Heroic manifests.
    *   **Hardware Detector**: Uses `udev` to discover gaming peripherals and verify `uaccess`.
    *   **Diagnostics**: Gathers system specs (CPU, GPU, RAM) and library support.
2.  **Managers**: Handle caching, TTLs, and data aggregation for each category.
3.  **Varlink Service**: The primary communication layer, exposing typed JSON-RPC over Unix sockets.
4.  **Security Layer**: Enforces access control based on the caller's identity (see `security.md`).

---

## 🛰️ Application Interaction Patterns

### 1. Polling (Status Queries)
Apps like system monitors or dashboard overlays poll the public interface to get the current state.
```mermaid
sequenceDiagram
    participant App
    participant contextd
    App->>contextd: GetActiveGame()
    contextd-->>App: { name: "Elden Ring", ... }
```

### 2. Subscription (Reactive Events)
Drivers or background services subscribe to updates to react in real-time (e.g. ambient lighting).
```mermaid
sequenceDiagram
    participant Driver
    participant contextd
    Driver->>contextd: SubscribeLightingContext()
    Note over contextd: Wait for state change...
    contextd-->>Driver: { main_color: "#FF00FF", ... }
```

### 3. Hinting (Cooperative Management)
Control apps signal their activity to avoid hardware conflicts via `RegisterController`.
```mermaid
sequenceDiagram
    participant Controller
    participant contextd
    Note right of Controller: Registered in config.toml
    Controller->>contextd: RegisterController(PID, "Lighting")
    contextd-->>Controller: Ok
    Note over contextd: Hint is now visible in ListDevices()
```
