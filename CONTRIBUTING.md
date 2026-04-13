# Contributing to `contextd`

## 🤲 How to Contribute

### 🐛 Reporting Bugs
- Use the GitHub Issue Tracker.
- Provide clear steps to reproduce the issue.
- Include your Linux distribution and kernel version.

### 💡 Feature Requests
- Open an issue titled `[Feature] Description`.
- Describe the use case and why it belongs in a generic context daemon.

### 🛠️ Pull Requests
1. **Fork the repo** and create your branch from `main`.
2. **Rust Style**: Ensure your code is formatted with `cargo fmt`.
3. **Varlink Changes**: If you modify the `.varlink` interface, ensure you run the generator to update the Rust bindings.
4. **Testing**: Run `cargo test` before submitting.
5. **Documentation**: Update `README.md` or files in `docs/` if you add new features.

## ⚖️ License
By contributing, you agree that your contributions will be licensed under the project's **MIT License**.
