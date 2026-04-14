#!/bin/bash
# Install git hooks for contextd development

# Get the root of the git repository
GIT_ROOT=$(git rev-parse --show-toplevel 2>/dev/null)

if [ -z "$GIT_ROOT" ]; then
    echo "Error: Not a git repository."
    exit 1
fi

HOOK_DIR="$GIT_ROOT/.git/hooks"
PRE_COMMIT="$HOOK_DIR/pre-commit"

echo "Installing pre-commit hook to $PRE_COMMIT..."

cat <<EOF > "$PRE_COMMIT"
#!/bin/bash
# Pre-commit hook to run fmt and clippy

# Run cargo fmt check
echo "Checking code formatting (cargo fmt)..."
cargo fmt -- --check
if [ \$? -ne 0 ]; then
    echo "ERROR: Code formatting check failed. Run 'cargo fmt' to fix."
    exit 1
fi

# Run cargo clippy
echo "Running system sanity checks (cargo clippy)..."
cargo clippy -- -D warnings
if [ \$? -ne 0 ]; then
    echo "ERROR: Clippy checks failed. Fix the warnings before committing."
    exit 1
fi

echo "Pre-commit checks passed!"
exit 0
EOF

chmod +x "$PRE_COMMIT"
echo "Git hooks installed successfully."
