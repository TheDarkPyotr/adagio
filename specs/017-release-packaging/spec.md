# Feature Specification: Binary Release Packaging

**Feature Branch**: `017-release-packaging`

**Created**: 2026-06-03

**Status**: Draft

**Input**: "I want a packaged binary release, including icon, desktop entry and a release plan, tag and section for github"

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Install Adagio from a Package (Priority: P1)

A Linux user downloads a `.deb` or `.rpm` package from the GitHub Releases page, installs it with their system package manager, and Adagio appears in their application launcher with the correct icon. They can launch it immediately — no manual PATH setup, no build from source.

**Why this priority**: This is the primary distribution mechanism. Without installable packages, Adagio is unreachable to non-developer users.

**Independent Test**: On a clean Ubuntu 22.04 machine with no Rust toolchain, run `sudo apt install ./adagio_*.deb`. Confirm Adagio appears in the GNOME app launcher with the correct icon and that double-clicking launches the app.

**Acceptance Scenarios**:

1. **Given** a `.deb` package on Ubuntu/Debian, **When** the user installs it with `apt` or `dpkg`, **Then** Adagio launches, the icon appears in the app launcher, and uninstalling with `apt remove adagio` removes all files cleanly.
2. **Given** a `.rpm` package on Fedora, **When** installed with `dnf`, **Then** the same clean install/uninstall cycle holds.
3. **Given** any supported Linux distribution without a matching native package, **When** the user downloads the `.tar.gz` archive and extracts it, **Then** a `README` inside explains how to run the binary and optionally install the desktop entry manually.
4. **Given** a macOS user, **When** they download and open the `.dmg`, **Then** they can drag Adagio to Applications and launch it immediately.

---

### User Story 2 — Find and Download a Release on GitHub (Priority: P1)

A user visits the Adagio GitHub repository, navigates to the Releases section, and immediately sees the latest stable release with clear version information, a human-readable changelog, and labelled download links for each platform.

**Why this priority**: GitHub Releases is the first place prospective users look. A well-structured release page is the project's public storefront.

**Independent Test**: Open the GitHub Releases page. Confirm it shows: version number, date, changelog, and separate download links for Linux (deb, rpm, tar.gz), macOS (dmg), and the source archive. Every download link must resolve to a valid file.

**Acceptance Scenarios**:

1. **Given** a published release, **When** a user visits the GitHub Releases page, **Then** they see: version tag (e.g., `v0.1.0`), release date, markdown changelog, and download links grouped by platform.
2. **Given** a user clicks a download link, **When** the download completes, **Then** the file is a valid package for the stated platform — not corrupt, correct format.
3. **Given** a release has been published, **When** a user clicks "Latest release" on the repository homepage, **Then** they reach the most recent stable release (not a pre-release or draft).

---

### User Story 3 — Cut a Release with a Single Git Tag (Priority: P1)

A maintainer creates and pushes a semver git tag (`v0.2.0`). Automatically, the CI pipeline builds release artifacts for all supported platforms, runs the full test suite, and publishes a GitHub Release with those artifacts attached — requiring no manual upload or manual release-page editing.

**Why this priority**: Manual release processes are error-prone and slow. Tag-triggered automation is the industry standard for reliable, reproducible releases.

**Independent Test**: Push a semver tag. Within 30 minutes, verify a release appears on GitHub with all artifacts attached and all CI checks green.

**Acceptance Scenarios**:

1. **Given** a semver tag is pushed to the repository, **When** the CI pipeline runs, **Then** it builds and attaches platform artifacts to a GitHub Release automatically — no manual steps required.
2. **Given** any test fails during the release build, **When** the pipeline detects the failure, **Then** the release is NOT published — no broken artifact is ever released.
3. **Given** a `v*.*.*` tag (no pre-release suffix), **When** the release is published, **Then** it is marked as the "latest" stable release on GitHub.
4. **Given** a `v*.*.*-rc*` or `v*.*.*-beta*` tag, **When** published, **Then** it is marked as a pre-release and does not replace the latest stable release.

---

### User Story 4 — System Integration on Linux (Priority: P2)

After installing the package on any standard Linux distribution, Adagio behaves as a first-class desktop citizen: it appears in the application menu, uses the correct icon at all standard sizes, and integrates cleanly with the XDG desktop environment.

**Why this priority**: Poor desktop integration (missing icon, broken menu entry) damages user trust in a sync client.

**Independent Test**: Install the `.deb` on Ubuntu 22.04 GNOME. Confirm: (a) Adagio icon appears in the app grid without pixellation, (b) the `.desktop` file contains correct `Name`, `Exec`, `Icon`, and `Categories` fields, (c) the app can be pinned to the taskbar.

**Acceptance Scenarios**:

1. **Given** the package is installed, **When** the user opens the application menu, **Then** Adagio appears under the "Internet" or "Accessories" category with the correct name and icon.
2. **Given** the package is installed, **When** the icon is displayed at sizes from 16 px to 256 px, **Then** it renders sharply at each size — not upscaled from a small raster.
3. **Given** the user enables "Start at login" in Adagio's preferences, **When** they log in, **Then** the daemon starts automatically via the existing XDG autostart mechanism.

---

### Edge Cases

- What if the build pipeline produces a binary that fails a post-build smoke test? The release must not be published; the CI job must fail visibly.
- What if a release tag was pushed by mistake? Contributing docs must explain how to delete a draft release and remove the tag.
- What if a user's distribution uses a different package format (Arch `PKGBUILD`, NixOS)? Out of scope for v1 — only `.deb`, `.rpm`, `.tar.gz`, and `.dmg` are supported.
- What if the macOS binary triggers a Gatekeeper warning (not notarized)? Notarization is deferred; the release notes must document this limitation explicitly.
- What if GitHub Actions minutes are exhausted? The release workflow must support being triggered manually (workflow_dispatch) as a fallback.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The CI pipeline MUST build release artifacts automatically when a git tag matching `v*.*.*` is pushed to the repository.
- **FR-002**: Release artifacts MUST include: Linux `.deb` (amd64), Linux `.rpm` (x86_64), Linux `.tar.gz` (amd64), and macOS `.dmg` (universal or arm64 + x86_64). Windows installer is best-effort in v1.
- **FR-003**: Each artifact MUST include all three Adagio components: the desktop GUI (`adagio-desktop`), the background daemon (`adagio-daemon`), and the CLI tool (`adagio-cli`).
- **FR-004**: The `.deb` and `.rpm` packages MUST install a valid XDG `.desktop` file so Adagio appears in the application launcher after installation.
- **FR-005**: The `.desktop` file MUST contain correct `Name`, `GenericName`, `Exec`, `Icon`, `Comment`, `Categories`, and `Terminal` fields, conforming to the freedesktop.org Desktop Entry Specification.
- **FR-006**: Icons MUST be installed at the standard XDG sizes (16, 24, 32, 48, 64, 128, 256, 512 px) in the system hicolor icon theme directory.
- **FR-007**: The pipeline MUST run the full test suite before building release artifacts; any failing test MUST abort the release and prevent artifact publication.
- **FR-008**: A GitHub Release MUST be created automatically for each tag, with: version number, release date, human-readable changelog, and all artifacts attached as downloadable assets.
- **FR-009**: Tags matching `v*.*.*` (no suffix) MUST publish as stable releases; tags with `-rc*`, `-beta*`, or `-alpha*` suffixes MUST publish as pre-releases.
- **FR-010**: Each artifact on the GitHub Releases page MUST have a corresponding SHA-256 checksum file so users can verify download integrity.
- **FR-011**: The release pipeline MUST be triggerable manually (workflow_dispatch) in addition to tag-push, to support re-runs and emergency releases.
- **FR-012**: The macOS `.dmg` MUST contain a drag-to-Applications installer layout with the app bundle and an Applications folder shortcut.
- **FR-013**: Package metadata (name, version, maintainer, description, homepage, license) MUST be consistent across all formats and sourced from the workspace `Cargo.toml`.

### Key Entities

- **Release**: A versioned snapshot identified by a semver tag. Attributes: version string, tag, release date, stability (stable / pre-release), changelog body, attached artifacts.
- **Artifact**: A distributable binary for a specific platform. Attributes: filename, platform, format (deb/rpm/tar.gz/dmg), SHA-256 checksum, size in bytes.
- **DesktopEntry**: The `.desktop` file installed with Linux packages. Attributes: Name, GenericName, Exec, Icon, Comment, Categories, Terminal, StartupNotify.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user with no Rust toolchain can install Adagio on Ubuntu 22.04 or Fedora 40 in under 2 minutes from first visiting the GitHub Releases page.
- **SC-002**: The full release pipeline (test → build → publish) completes in under 30 minutes from tag push to artifacts available on GitHub.
- **SC-003**: All release artifacts include a SHA-256 checksum that verifies correctly — zero checksum mismatches across all published artifacts.
- **SC-004**: The Adagio icon renders without pixellation at all standard desktop sizes (16 px to 256 px) on GNOME and KDE.
- **SC-005**: Zero broken releases published — every artifact attached to a stable GitHub Release must be installable and launchable on the stated platform.
- **SC-006**: The GitHub Releases page is self-explanatory — a first-time visitor can identify the correct download for their platform without reading external documentation.

## Assumptions

- All icon assets already exist in `icons/` in SVG and multi-size PNG formats; no new design work is required.
- GitHub Actions is the CI/CD platform; the release workflow extends the existing `ci.yml`.
- The project version is the single source of truth defined in the workspace `Cargo.toml`; all package metadata is derived from it.
- A macOS GitHub Actions runner is available for building macOS artifacts.
- Code signing and notarization for macOS are explicitly out of scope for v1 and will be documented as a known limitation in the release notes.
- The changelog is auto-generated from git history between tags using conventional commit prefixes; no manual editing is required per release.
- Semantic versioning (semver) is followed: `MAJOR.MINOR.PATCH`. The first public release will be `v0.1.0`.
- The `.deb` package already declares `libayatana-appindicator3-1 | libappindicator3-1` as a runtime dependency (already wired in `tauri.conf.json` from feature 015).
- The `adagio-desktop` Tauri app produces the installable bundle; `adagio-daemon` and `adagio-cli` are standalone binaries packaged alongside it.
