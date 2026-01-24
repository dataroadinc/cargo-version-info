---
name: commit
description:
  Create commits following Angular Conventional Commits format with
  proper scope naming for consistent changelog generation
---

# Commit Skill

Use this skill when creating git commits. All commits must follow
Angular Conventional Commits format because:

1. Git hooks enforce it (commit will be rejected otherwise)
2. Cocogitto parses commits to generate release changelogs
3. The `cargo version-info changelog` command relies on this format

## Commit Message Format

```text
<type>(<scope>): <subject>

[optional body]

[optional footer]
```

**Requirements:**

- Type: lowercase, from allowed list
- Scope: mandatory, lowercase, describes what changed
- Subject: lowercase start, imperative mood, no period
- Breaking changes: add exclamation mark before colon (e.g. feat(api)!:)

## Allowed Types

### Angular standard types

| Type       | Purpose                          | In Changelog |
| ---------- | -------------------------------- | ------------ |
| `feat`     | New feature                      | Yes          |
| `fix`      | Bug fix                          | Yes          |
| `docs`     | Documentation only               | Yes          |
| `refactor` | Code change (no feat/fix)        | Yes          |
| `perf`     | Performance improvement          | Yes          |
| `build`    | Build system changes             | Yes          |
| `style`    | Formatting, whitespace           | No           |
| `test`     | Adding/fixing tests              | No           |
| `ci`       | CI/CD configuration              | No           |

### Extended types (not in original Angular spec)

These are widely adopted extensions configured in `cog.toml`:

| Type       | Purpose                          | In Changelog |
| ---------- | -------------------------------- | ------------ |
| `chore`    | Maintenance, deps, tooling       | No           |
| `revert`   | Revert previous commit           | Yes          |

## Scope Naming Guidelines

Scopes should be consistent to group related changes in changelogs.

### Command scopes

Use the command name as scope when modifying a specific command:

- `bump` - the bump command
- `changelog` - changelog generation
- `release-page` - release page generation
- `badge` - badge generation
- `pr-log` - PR log generation
- `current`, `latest`, `next`, `dev`, `tag`, `changed`, etc.

### Component scopes

- `version` - version string handling, version bumps
- `github` - GitHub API integration
- `git` - git operations (non-command specific)

### Infrastructure scopes

- `deps` - dependency updates
- `ci` - CI/CD workflows (use with `ci:` or `chore:` type)
- `config` - configuration files
- `hooks` - git hooks, Sloughi

### Documentation scopes

- `readme` - README.md changes
- `claude` - CLAUDE.md, Claude Code skills
- `contributing` - CONTRIBUTING.md

### Testing scopes

- `tests` - general test changes
- `windows` / `macos` / `linux` - platform-specific fixes

## Examples

### Good commit messages

```text
feat(bump): add Cargo.lock update to version bump
fix(changelog): handle missing start tag gracefully
docs(readme): update installation instructions
refactor(github): extract API client into module
test(bump): add tests for hunk-level staging
chore(deps): update octocrab to 0.49.5
ci(workflows): add release binary uploads
perf(version): cache parsed version strings
feat(badge)!: change badge format to shields.io
```

### Bad commit messages

```text
feat: add feature              # missing scope
Fix(bump): Fix bug             # uppercase type and subject
feat(bump): Add feature.       # uppercase subject, has period
updated the readme             # wrong format entirely
feat(bump) add feature         # missing colon
feat(Bump): add feature        # uppercase scope
```

## Breaking Changes

For breaking changes, add an exclamation mark after the scope:

```text
feat(api)!: remove deprecated endpoint

BREAKING CHANGE: The /v1/status endpoint has been removed.
Use /v2/health instead.
```

This will appear prominently in the changelog.

## Commit Process

1. Stage your changes: `git add <files>`
2. Commit with message: `git commit -m "type(scope): subject"`
3. Hooks automatically run:
   - pre-commit: fmt and clippy checks
   - commit-msg: validates conventional commit format
   - post-commit: verifies signature

If the commit is rejected, fix the issue and try again.

## Multi-line Commits

For complex changes, use a body:

```bash
git commit -m "feat(bump): add README badge updates

Updates version badges in README.md when bumping.
Searches for crates.io and docs.rs badge patterns
and replaces version numbers.

Closes #42"
```

## Checking Recent Commits

To see the commit style used in this repo:

```bash
git log --oneline -20
```
