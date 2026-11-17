# Contributing to `contextd`

## 🤲 How to Contribute

### 🐛 Reporting Bugs
- Use the GitHub Issue Tracker.
- Provide clear steps to reproduce the issue.
- Include your Linux distribution and kernel version.

### 💡 Feature Requests
- Open an issue titled `[Feature] Description`.
- Describe the use case and why it belongs in a generic context daemon.

### 💻 Development Environment
To maintain code quality, we use git hooks to run `cargo fmt` and `cargo clippy` before every commit. You can set them up with:
```bash
bash scripts/install-hooks.sh
```
This ensures your PRs are clean and reduces CI failures.

### 🛠️ Pull Requests
1. **Fork the repo** and create your branch from `master`.
2. **Setup**: Run `./scripts/install-hooks.sh` to install local git hooks that automatically verify formatting and linting before commits.
3. **Rust Style**: Ensure your code is formatted with `cargo fmt`.
4. **Varlink Changes**: Interfaces live in `contextd-client/interfaces/`. Bindings are regenerated automatically by `contextd-client/build.rs`, and the daemon uses the same generated code. Prefer adding optional fields and new methods over changing existing ones.
5. **Testing**: Run `cargo test --workspace` before submitting. `tests/daemon.rs` runs the real daemon on temporary sockets; add a case there for behaviour a client can observe.
6. **Documentation**: Update `README.md` or files in `docs/` if you add new features.

## ⚖️ License
By contributing, you agree that your contributions will be licensed under the project's **MIT License**.
