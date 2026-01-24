# Plan

Planned improvements for cargo-version-info.

## Pending

- [ ] **Sign commits created by `bump` command**: The `bump` command
      creates commits that are not signed. Repos that require signed
      commits (via GitHub branch protection rules) reject pushes of
      unsigned commits. The bump command should respect the user's
      git signing configuration (`commit.gpgsign`, `user.signingkey`,
      etc.) and create signed commits when configured.
