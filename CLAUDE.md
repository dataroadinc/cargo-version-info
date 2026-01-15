# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when
working with code in this repository.

## Related Projects

This crate is part of a family of Rust projects that share the same
coding standards, tooling, and workflows:

Cargo plugins:

- `cargo-fmt-toml` - Format and normalize Cargo.toml files
- `cargo-nightly` - Nightly toolchain management
- `cargo-plugin-utils` - Shared utilities for cargo plugins
- `cargo-propagate-features` - Propagate features to dependencies
- `cargo-version-info` - Dynamic version computation

Other Rust crates:

- `dotenvage` - Environment variable management

All projects use identical configurations for rustfmt, clippy,
markdownlint, cocogitto, and git hooks. When making changes to
tooling or workflow conventions, apply them consistently across
all repositories.

## Project Overview

`cargo-version-info` is a Cargo subcommand providing unified version
management across CI/CD, Rust code, and shell scripts. It replaces
scattered version logic in workflows, bash scripts, and build.rs files.

Key commands:

- `next` - Calculate next patch version from latest GitHub release
- `current` - Get current version from Cargo.toml
- `latest` - Get latest published GitHub release version
- `dev` - Generate dev version from current git SHA
- `bump` - Bump version in Cargo.toml and commit changes
- `changed` - Check if version changed since last git tag
- `changelog` - Generate changelog from conventional commits
- `release-page` - Generate complete release page with badges

Published to crates.io at https://crates.io/crates/cargo-version-info

## Build Commands

```bash
# Build
cargo build

# Run directly (during development)
cargo run -- version-info <command> [OPTIONS]

# Run after installation
cargo version-info <command> [OPTIONS]
```

## Testing and Linting

```bash
# Run tests (single-threaded required)
cargo test -- --test-threads=1

# Run a single test
cargo test test_name -- --test-threads=1

# Format check (requires nightly)
cargo +nightly fmt --all -- --check

# Format code
cargo +nightly fmt --all

# Clippy (requires nightly)
cargo +nightly clippy --all-targets --all-features -- -D warnings -W missing-docs
```

## Code Style

- **Rust Edition**: 2024, MSRV 1.92.0
- **Formatting**: Uses nightly rustfmt with vertical imports grouped
  by std/external/crate (see `rustfmt.toml`)
- **Clippy**: Nightly with strict settings (max 120 lines/function,
  nesting threshold 5)
- **Disallowed variable names**: foo, bar, baz, qux, i, n
- **Documentation**: All public items must have docs (`-W missing-docs`)

## Architecture

### Source Structure

- `src/main.rs` - CLI entry point with clap argument parsing
- `src/lib.rs` - Library definition
- `src/version.rs` - Core version parsing, formatting, comparison
- `src/github.rs` - GitHub API integration using octocrab
- `src/commands/` - 21 command implementations, each with its own
  Args struct

### Bump Command Architecture

The `bump` subcommand (`src/commands/bump/`) is the most sophisticated:

- `mod.rs` - Main orchestration
- `version_update.rs` - TOML manipulation with toml_edit
- `commit.rs` - Git commit orchestration
- `diff.rs` - Diff generation and hunk-level filtering
- `index.rs` - Git index operations using gix
- `tree.rs` - Git tree building

Key feature: Hunk-level selective staging commits only version-related
changes, leaving other uncommitted work untouched.

### Key Dependencies

- `clap` - CLI argument parsing with derive macros
- `octocrab` - GitHub API client (with rustls)
- `gix` - Pure-Rust git library (gitoxide)
- `toml_edit` - Preserves TOML formatting during edits
- `tokio` - Async runtime for GitHub API calls
- `cargo_plugin_utils` - Shared utilities for cargo plugins

## Version Management

Use `cargo version-info bump` for version management. This command
updates Cargo.toml and creates a commit, but does NOT create tags
(tags are created by CI after tests pass).

```bash
cargo version-info bump --patch   # 0.0.1 -> 0.0.2
cargo version-info bump --minor   # 0.1.0 -> 0.2.0
cargo version-info bump --major   # 1.0.0 -> 2.0.0
```

**Do NOT use `cog bump`** - it creates local tags which conflict
with CI's tag creation workflow.

**Workflow:**

1. Create PR with version bump commit
2. Merge PR to main
3. CI detects version change, creates tag, publishes release

### Build Version Priority

Version is computed dynamically via `build.rs`:

1. `BUILD_VERSION` env var (CI)
2. Cargo.toml version + git SHA
3. Fallback: `0.0.0-dev-<short-sha>`

## Git workflow

- Commits follow Angular Conventional Commits:
  `<type>(<scope>): <subject>`
- Types: feat, fix, docs, refactor, test, style, perf, build, ci,
  chore, revert
- Use lowercase for type, scope, and subject start
- Never bypass git hooks with `--no-verify`
- Never execute `git push` - user must push manually
- Prefer `git rebase` over `git merge` for linear history

Git hooks in `.githooks/` are auto-installed via `sloughi` during
build.

## Markdown formatting

- Maximum line length: 70 characters
- Use `-` for unordered lists (not `*` or `+`)
- Use sentence case for headers (not Title Case)
- Indent nested lists with 2 spaces
- Surround lists and code blocks with blank lines

### Markdown linting

Configuration is in `.markdownlint.json`:

- Line length: 70 characters (MD013)
- Code blocks: unlimited line length

```bash
markdownlint '**/*.md' --ignore node_modules --ignore target
```
