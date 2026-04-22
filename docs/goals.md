# Design Goals

The development of `contextd` is guided by three core principles: **Simplicity**, **Efficiency**, and **Interoperability**.

## 1. Simplicity ("The Dumb Daemon")
The daemon should be as simple as possible. It gathers data and serves it.
- **No Complex Logic**: Policy decisions (e.g., "What color should the keyboard be when Elden Ring is running?") belong in user-level applications, not the daemon.
- **Zero-Conf by Default**: Works out of the box for most users without requiring manual configuration.

## 2. Efficiency
`contextd` should have a negligible footprint.
- **Poll-on-Demand**: No background scanning or monitoring threads unless strictly necessary for an interface (like the RGB observer).
- **TTL Caching**: Short-lived caches prevent high-frequency callers from generating system load.
- **No Heavy Dependencies**: Avoids D-Bus or complex IPC frameworks in favor of standard Unix sockets and JSON.

## 3. Interoperability
The daemon acts as a bridge between disparate parts of the Linux ecosystem.
- **Launcher Agnostic**: Treats Steam, Lutris, and Heroic as first-class citizens.
- **Language Agnostic**: Varlink ensures that a C app, a Python script, or a Rust service can all talk to `contextd` with equal ease.
- **Cooperative**: Provides the "Bulletin Board" (Controller Hints) to help independent hardware tools coordinate their activity.
