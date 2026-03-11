# 🤖 Agents' Guide to `massive-adtack`

Welcome, Agent. This document is designed to help you understand the codebase, architectural decisions, and development workflows for **Massive AD-tack**.

## 📂 Project Structure

The project follows a modular Rust application structure:

- **`src/main.rs`**: The entry point. It sets up the `clap` CLI parser and dispatches commands to their respective modules. Keep this file slim.
- **`src/commands/`**: Contains the logic for each subcommand.
  - **`mod.rs`**: Exports the command modules.
  - **`users.rs`**: Handles user creation (`add`), listing (`list`), and deletion (`rm`). This is the most complex module.
  - **`groups.rs`**: Handles group-related commands (`add` and `list`).
  - **`check.rs`**: Implements the `check` command for server connectivity and feature discovery.
  - **`config.rs`**: Simple command to dump current configuration.
- **`src/oids.rs`**: Central registry for OIDs used in the project (e.g., `1.2.840.113556.1.4.319` for Paged Results). Data is loaded from `src/oids.txt`.
- **`src/dn.rs`**: Custom `DistinguishedName` type for safe DN handling.
- **`src/naming.rs`**: Utilities for generating random names and attributes.

## 🏗️ Architectural Principles

1.  **Async First**: All LDAP operations must be asynchronous using `tokio` and `ldap3`. We need to handle high concurrency.
2.  **Performance**:
    -   Avoid unnecessary allocations in loops.
    -   Reuse LDAP connections where possible (though currently `ldap3` might require careful handling of pool/connection lifecycle).
    -   Use `indicatif` for progress bars to give feedback without flooding stdout.
3.  **Safety**:
    -   **Search-then-Action**: For destructive operations like `rm`, always search and count/confirm before deleting.
    -   **Dry Run**: Implement `--dry-run` for all state-changing commands.
4.  **Error Handling**:
    -   Use `anyhow::Result` for application-level error handling.
    -   Contextualize errors: `context("Failed to bind to LDAP")?`.
    -   Distinguish between "retriable" errors (timeouts) and "fatal" errors (auth failure).

## 🛠️ Development Workflow

### Prerequisites
-   **Rust Toolchain**: Stable channel.
-   **Local Samba AD**: A Dockerized Samba AD DC (e.g., `instantlinux/samba-dc`) running on `localhost:10389`.

### Common Commands
```bash
# Check code style
cargo fmt --all -- --check

# Linting
cargo clippy -- -D warnings

# Run specific command (example)
cargo run -- users list --ldap-filter "(cn=admin*)"

# Run tests
cargo test
```

## 📝 Git Commit Guidelines

Follow the [seven rules of a great Git commit message](https://cbea.ms/git-commit/):

1.  **Separate subject from body with a blank line**
2.  **Limit the subject line to 50 characters**
3.  **Capitalize the subject line**
4.  **Do not end the subject line with a period**
5.  **Use the imperative mood in the subject line**
    - Think: "If applied, this commit will *[your subject line]*"
    - ✅ "Add user deletion command"
    - ❌ "Added user deletion command"
6.  **Wrap the body at 72 characters**
7.  **Use the body to explain *what* and *why* vs. *how***

### Example

```
Add bulk user deletion with safety checks

Implement users rm command with search-then-delete strategy.
Users can preview deletions with --dry-run before executing.
Confirmation prompt prevents accidental bulk deletions unless
--no-confirm flag is used for scripting scenarios.
```

## 🧪 Testing Guidelines

Since we interact with a live AD, unit tests for network logic are hard. Focus on:
1.  **Unit Tests**: Test logic that *doesn't* require a network (e.g., DN parsing, name generation, CLI argument validation).
2.  **Integration Tests**: If a test environment is available, script `cargo run` commands to verify end-to-end flows.

## 📝 Key Files for Context
-   `Cargo.toml`: Dependency versions.
-   `src/lib.rs`: (If present) Shared library code.

---
*Happy Coding!*
