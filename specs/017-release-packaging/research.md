# Research: Binary Release Packaging

**Feature**: 017-release-packaging
**Phase**: 0 — Pre-design research
**Date**: 2026-06-03

---

## Finding 1: Tauri 2 Bundler Covers All Required Package Formats

**Decision**: Use `tauri build` as the sole packaging tool for all binary formats.

**Rationale**: Tauri 2's bundler already produces `.deb`, `.rpm`, `.AppImage`, and `.dmg` from a single `tauri build` invocation, controlled by `tauri.conf.json → bundle`. The bundle config in the repo already declares:
- Icons (`icons/32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`, `icon.ico`)
- `deb.depends` (libayatana-appindicator3-1 — added in feature 015)

The bundler also automatically generates the XDG `.desktop` file for Linux packages, installs icons into `/usr/share/icons/hicolor/`, and creates the macOS `.dmg` with drag-to-Applications layout. No custom packaging scripts are required.

**Tauri bundle targets relevant to this feature**:
- `deb` — Debian/Ubuntu `.deb` (built on Linux runner)
- `rpm` — Fedora/RHEL `.rpm` (built on Linux runner)
- `app` + `dmg` — macOS `.dmg` (built on macOS runner)

**`adagio-daemon` and `adagio-cli` packaging**: These are standalone Rust binaries, not Tauri apps. They must be bundled separately alongside the Tauri app. For `.deb`/`.rpm`, Tauri supports `bundle.externalBin` to include extra binaries. The daemon and CLI will be declared as external binaries so they are packaged into the same installer.

**Alternatives considered**:
- Custom Docker-based packaging (e.g., `fpm`) — rejected: adds complexity; Tauri already handles this natively.
- Separate packages for daemon and CLI — rejected: requires users to install 3 packages; a single unified package is simpler.

---

## Finding 2: GitHub Actions Release Workflow

**Decision**: New workflow file `.github/workflows/release.yml` triggered on `v*.*.*` tags. Uses `softprops/action-gh-release` to create the GitHub Release and attach artifacts.

**Workflow structure**:
```
release.yml
  on: push tags v*.*.*
  
  jobs:
    test          ← full test suite (reuses ci.yml logic)
    build-linux   ← tauri build on ubuntu-latest → .deb, .rpm, .tar.gz
    build-macos   ← tauri build on macos-latest → .dmg
    publish       ← create GitHub Release, attach all artifacts + checksums
```

The `publish` job runs only if all build jobs pass, enforcing FR-007 (no broken release published).

**Pre-release detection**: The `softprops/action-gh-release` action accepts `prerelease: ${{ contains(github.ref_name, '-rc') || contains(github.ref_name, '-beta') || contains(github.ref_name, '-alpha') }}` to satisfy FR-009.

**Manual trigger**: `workflow_dispatch` with an optional `tag` input satisfies FR-011.

**Alternatives considered**:
- `goreleaser` — excellent tool but Rust/Tauri-native; Tauri CLI is already integrated and simpler.
- Extending `ci.yml` — rejected: release and CI have different trigger conditions and lifecycles; separation is cleaner.

---

## Finding 3: Changelog Generation with `git-cliff`

**Decision**: Use [`git-cliff`](https://github.com/orhun/git-cliff) to auto-generate the changelog body for each GitHub Release.

**Rationale**: `git-cliff` reads conventional commit messages (`feat:`, `fix:`, `chore:`, etc.) and produces a structured markdown changelog. It can output only the diff between the previous tag and the current tag, which is exactly the per-release changelog body needed for FR-008.

**Usage in release workflow**:
```bash
git-cliff --latest --strip header > CHANGELOG_BODY.md
```
The output is passed as the `body` input to `softprops/action-gh-release`.

**`git-cliff.toml` configuration**: A `cliff.toml` will be added to the repo root with Adagio's commit type mappings (`feat` → Features, `fix` → Bug Fixes, etc.).

**Alternatives considered**:
- Manual CHANGELOG.md maintained by hand — rejected: error-prone, adds release friction.
- `auto-changelog` — similar capability; `git-cliff` has better Rust ecosystem integration and is used widely in Tauri projects.

---

## Finding 4: SHA-256 Checksum Generation

**Decision**: Generate `.sha256` sidecar files in the `publish` job using `sha256sum` (Linux) / `shasum -a 256` (macOS), normalized with a cross-platform shell script.

**Implementation**: In the `publish` job (runs on Ubuntu), all artifacts are collected and checksummed:
```bash
for f in dist/*; do sha256sum "$f" >> checksums.txt; done
```
Both `checksums.txt` (all checksums in one file) and individual `.sha256` sidecar files are attached to the release (FR-010).

---

## Finding 5: Desktop Entry and Icon Installation

**Decision**: Use Tauri's default desktop entry generation; supplement with a custom template for fields Tauri doesn't set by default (GenericName, Comment, Categories).

**Tauri's `tauri.conf.json` `bundle.linux.deb` and `bundle.linux.rpm`** support:
- `desktopTemplate`: path to a custom `.desktop` template file
- Tauri fills `Name`, `Exec`, `Icon`, `Version` automatically

**Custom template** (`bundler/adagio.desktop.template`):
```ini
[Desktop Entry]
Type=Application
Name=Adagio
GenericName=Cloud File Sync
Comment=Sync files with your Nextcloud server
Exec=adagio-desktop
Icon=adagio
Terminal=false
Categories=Network;FileTransfer;
StartupNotify=true
```

**Icon installation**: Tauri's bundler installs icons from `bundle.icon` into `/usr/share/icons/hicolor/<size>/apps/`. The current `bundle.icon` array references `32x32.png`, `128x128.png`, and `128x128@2x.png`. We will expand this to include all standard XDG sizes from the existing `icons/png/` directory (16, 24, 32, 48, 64, 128, 256, 512 px) to satisfy FR-006.

---

## Finding 6: Version Sourcing

**Decision**: The workspace `Cargo.toml` `version` field is the single source of truth. Tauri reads it automatically for `tauri.conf.json → version`. The GitHub tag `v{version}` is enforced by the workflow (it fails if the tag doesn't match the Cargo version).

**Version enforcement in CI**:
```bash
CARGO_VERSION=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="adagio-desktop") | .version')
TAG_VERSION="${GITHUB_REF_NAME#v}"
if [ "$CARGO_VERSION" != "$TAG_VERSION" ]; then
  echo "Tag version $TAG_VERSION does not match Cargo version $CARGO_VERSION"
  exit 1
fi
```

This ensures releases can only be cut when the code version is bumped first.

---

## Finding 7: `externalBin` for Daemon and CLI

**Decision**: Declare `adagio-daemon` and `adagio-cli` as `bundle.externalBin` in `tauri.conf.json` so Tauri packages them into the installer alongside `adagio-desktop`.

**Format**: Tauri's `externalBin` expects platform-suffixed binaries at a specified path relative to the crate root:
```json
{
  "bundle": {
    "externalBin": [
      "binaries/adagio-daemon",
      "binaries/adagio-cli"
    ]
  }
}
```
The CI build step copies the compiled `adagio-daemon` and `adagio-cli` binaries into `crates/adagio-desktop/binaries/` before running `tauri build`, satisfying FR-003.

**Alternatives considered**:
- Three separate packages (desktop, daemon, CLI) — rejected: install friction for users.
- Shipping only the desktop app with daemon embedded — rejected: daemon is a separate process by design.

---

## Summary Table

| # | Decision | Impact |
|---|----------|--------|
| 1 | Tauri 2 bundler for all formats + externalBin for daemon/CLI | **Critical** — single build command produces all formats |
| 2 | `.github/workflows/release.yml` with build matrix + `softprops/action-gh-release` | **Critical** — automated publish pipeline |
| 3 | `git-cliff` for per-release changelog | Medium — clean release notes |
| 4 | `sha256sum` sidecar files in publish job | Medium — user integrity verification |
| 5 | Custom `.desktop` template + expanded icon list | Medium — XDG compliance |
| 6 | Tag-vs-Cargo-version enforcement | Medium — prevents mismatched releases |
| 7 | `externalBin` for daemon + CLI | **Critical** — FR-003 (all three components in one install) |
