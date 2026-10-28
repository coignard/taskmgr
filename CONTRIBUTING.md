# Contributing to Task Manager

## Development Setup

```bash
git clone https://github.com/coignard/taskmgr
cd taskmgr
cargo build --workspace
cargo test --workspace
cargo xtask bundle
```

Task Manager itself builds and tests on any operating system. The macOS shell needs a Mac, so elsewhere leave it out:

```bash
cargo build --workspace --exclude taskmgr-macos
cargo test --workspace --exclude taskmgr-macos
```

## Guidelines

### Code Style

- Run `cargo fmt` before committing
- Fix all `cargo clippy` warnings
- Add tests for new features

### Commit Messages

```
type(scope): brief description

Detailed explanation (if needed)
```

Types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`

### Pull Requests

1. Fork the repository
2. Create feature branch (`git checkout -b feat/describe-your-feature`)
3. Commit changes
4. Push to branch
5. Open pull request

### Testing

CI runs the same checks as these commands:

```bash
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --target aarch64-apple-darwin -- -D warnings
cargo clippy --workspace --exclude taskmgr-macos --all-targets --all-features -- -D warnings
cargo deny check
```
