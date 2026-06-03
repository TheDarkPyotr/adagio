# Contributing to Adagio

## Cutting a Release

### Prerequisites

- You have push access to the repository.
- The `main` branch is in a releasable state (all tests green).

### Steps

1. **Bump the version** in `Cargo.toml` (workspace `[package]` section):
   ```toml
   version = "0.2.0"
   ```
   Also update `crates/adagio-desktop/tauri.conf.json` → `"version"` to match.

2. **Commit the version bump**:
   ```bash
   git commit -am "chore: bump version to 0.2.0"
   git push origin main
   ```

3. **Push a semver tag** — this triggers the release pipeline automatically:
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```
   Within ~30 minutes a GitHub Release appears with all artifacts attached.

4. **For a pre-release** (RC, beta), use a suffix:
   ```bash
   git tag v0.2.0-rc1
   git push origin v0.2.0-rc1
   ```
   The pipeline marks this as a pre-release and does not update the `latest` badge.

### Recovering from a botched release

If a tag was pushed by mistake or the pipeline produced bad artifacts:

1. **Delete the draft release** on the GitHub Releases page (if published, edit → delete assets).
2. **Delete the remote tag**:
   ```bash
   git push origin --delete v0.2.0
   ```
3. **Delete the local tag** (if needed):
   ```bash
   git tag -d v0.2.0
   ```
4. Fix the issue, then re-tag and push.

### Manual re-trigger

If you need to re-run the release pipeline without re-pushing a tag (e.g., after a transient GitHub Actions failure):

1. Go to **Actions → Release** in the GitHub UI.
2. Click **Run workflow**.
3. Enter the tag to release (e.g., `v0.2.0`) — the tag must already exist.

### Tag naming conventions

| Pattern | Release type | `latest` updated |
|---------|-------------|-----------------|
| `v1.2.3` | Stable | Yes |
| `v1.2.3-rc1` | Pre-release (release candidate) | No |
| `v1.2.3-beta1` | Pre-release (beta) | No |
| `v1.2.3-alpha1` | Pre-release (alpha) | No |
