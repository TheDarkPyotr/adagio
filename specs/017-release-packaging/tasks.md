# Tasks: Binary Release Packaging

**Input**: Design documents from `specs/017-release-packaging/`

**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, contracts/release-workflow.md ✅

**Note**: This feature is CI/CD config + Tauri packaging config — no new Rust crates. Tests are CI-level smoke tests (package validity checks) rather than unit tests.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no shared dependencies)
- **[Story]**: Which user story this task belongs to (US1–US4)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Directory scaffolding and one-time tooling needed before any config work.

- [x] T001 Create directory `crates/adagio-desktop/bundler/` for desktop entry template and any future bundle assets
- [x] T002 [P] Add `git-cliff` to CI toolchain: confirm `cargo install git-cliff` step works on ubuntu-latest and document version pin in `cliff.toml`

**Checkpoint**: Bundler directory exists; git-cliff installs cleanly in a test CI step.

---

## Phase 2: Foundational — Tauri Bundle Configuration (Blocking)

**Purpose**: Config changes that ALL user stories depend on. Must be complete before the release workflow can produce valid artifacts.

- [x] T003 Expand `bundle.icon` in `crates/adagio-desktop/tauri.conf.json` to include all XDG sizes: add paths for `icons/png/adagio-icon-16.png`, `icons/png/adagio-icon-24.png`, `icons/png/adagio-icon-48.png`, `icons/png/adagio-icon-64.png`, `icons/png/adagio-icon-256.png`, `icons/png/adagio-icon-512.png` alongside the existing 32×32, 128×128, 128×128@2×, icns, and ico entries
- [x] T004 Add `bundle.externalBin` array to `crates/adagio-desktop/tauri.conf.json` declaring `"binaries/adagio-daemon"` and `"binaries/adagio-cli"` so Tauri packages daemon and CLI into each installer
- [x] T005 Add `bundle.linux.deb.desktopTemplate` pointing to `"bundler/adagio.desktop.template"` and `bundle.linux.rpm.desktopTemplate` pointing to the same file in `crates/adagio-desktop/tauri.conf.json`
- [x] T006 Create `crates/adagio-desktop/bundler/adagio.desktop.template` with the exact fields from the desktop entry contract: `Type`, `Name`, `GenericName`, `Comment`, `Exec`, `Icon`, `Terminal`, `Categories`, `StartupNotify`
- [x] T007 Create `cliff.toml` at repo root with Adagio commit-type mappings: `feat` → Features, `fix` → Bug Fixes, `ci` → CI Changes, `docs` → Documentation, `perf` → Performance, `refactor` → Refactoring; set `tag_pattern = "v[0-9].*"` to scope changelog to semver tags

**Checkpoint**: `tauri build` reads updated config without errors; `cliff.toml` generates a valid changelog from local git history (`git cliff --unreleased --output /dev/null`).

---

## Phase 3: User Story 3 — Tag-Triggered Release Pipeline (Priority: P1) 🎯 MVP

**Goal**: A maintainer pushes a semver tag; CI builds all artifacts and publishes a GitHub Release automatically, with no manual steps.

**Independent Test**: Push a test pre-release tag (`v0.1.0-rc1`) to a fork. Verify the `release.yml` workflow runs, all jobs pass, a pre-release appears on the GitHub Releases page with artifacts attached, and the `latest` stable release is not replaced.

- [x] T008 [US3] Create `.github/workflows/release.yml` with `on.push.tags: ['v*.*.*']` and `on.workflow_dispatch` triggers; add `permissions: contents: write` at workflow level for artifact publishing
- [x] T009 [US3] Add `test` job in `release.yml` that runs `cargo test --workspace` on `ubuntu-latest` and `macos-latest` in a matrix, mirroring the existing `ci.yml` test step; both matrix legs must pass before any build job starts
- [x] T010 [US3] Add version enforcement step in `release.yml` (runs after checkout, before `build-linux`/`build-macos`): extract `CARGO_VERSION` from `cargo metadata`, strip `v` prefix from `GITHUB_REF_NAME`, fail with diagnostic if they differ
- [x] T011 [US3] Add `build-linux` job in `release.yml` that runs on `ubuntu-latest`, depends on `test`, installs Tauri prerequisites (`libgtk-3-dev`, `libayatana-appindicator3-dev`, `webkit2gtk`), copies `adagio-daemon` and `adagio-cli` binaries into `crates/adagio-desktop/binaries/` after `cargo build --release`, then runs `cargo tauri build --bundles deb,rpm,app-image`
- [x] T012 [US3] Add `build-macos` job in `release.yml` that runs on `macos-latest`, depends on `test`, copies daemon/CLI binaries into `binaries/`, runs `cargo tauri build --bundles dmg`; targets `universal-apple-darwin` (arm64 + x86_64 lipo) if feasible, else arm64 only
- [x] T013 [US3] Add checksum step to `build-linux` and `build-macos` jobs: after `tauri build`, iterate over `src-tauri/target/release/bundle/**/*.{deb,rpm,tar.gz,dmg}` and write SHA-256 to individual `.sha256` sidecar files; also append all checksums to `checksums.txt`
- [x] T014 [US3] Add `publish` job in `release.yml` that depends on both `build-linux` and `build-macos`; downloads artifacts from both jobs using `actions/download-artifact`; calls `softprops/action-gh-release` with `files: dist/**`, `generate_release_notes: false` (uses cliff), `prerelease: ${{ contains(github.ref_name, '-rc') || contains(github.ref_name, '-beta') || contains(github.ref_name, '-alpha') }}`, and `make_latest: ${{ !contains(github.ref_name, '-') }}`
- [x] T015 [US3] Wire `git-cliff` into the `publish` job: install `git-cliff`, run `git cliff --current --output CHANGELOG_RELEASE.md` to generate per-release notes, pass the file content to `softprops/action-gh-release` `body_path` field

**Checkpoint**: Phase 3 complete when the full workflow runs end-to-end on a test tag and a GitHub Release appears with all five artifact types attached (deb, rpm, tar.gz, dmg, checksums.txt).

---

## Phase 4: User Story 1 — Installable Packages (Priority: P1)

**Goal**: A Linux user downloads the `.deb` or `.rpm`, installs it with their package manager, and Adagio appears in the app launcher ready to run — all three components (desktop, daemon, CLI) installed.

**Independent Test**: On Ubuntu 22.04 (no Rust toolchain), run `sudo dpkg -i adagio_*.deb` on the artifact from a release. Confirm: (a) `dpkg -L adagio` lists `adagio-desktop`, `adagio-daemon`, `adagio-cli` binaries; (b) `adagio-desktop &` launches the UI; (c) `adagio-cli status` runs without error; (d) `sudo apt remove adagio` leaves no orphan files.

- [x] T016 [US1] Add a CI smoke-test step to `build-linux` in `release.yml`: after `tauri build`, install the produced `.deb` via `sudo dpkg -i` in the runner and verify `dpkg -L adagio` lists all three binaries; fail the job if any are missing
- [x] T017 [US1] Add a `tar.gz` packaging step to `build-linux` in `release.yml`: create `adagio_${VERSION}_amd64.tar.gz` containing `adagio-desktop`, `adagio-daemon`, `adagio-cli`, and a `README.txt` explaining how to run manually and optionally install the `.desktop` file
- [x] T018 [US1] Write `crates/adagio-desktop/bundler/tarball-README.txt` with: launch instructions (`./adagio-desktop`), optional XDG desktop integration steps (`cp adagio.desktop ~/.local/share/applications/`), and a note that `.deb`/`.rpm` packages handle this automatically

**Checkpoint**: The `.deb` installs cleanly in the CI runner, all three binaries are present, and the tarball README gives clear manual instructions.

---

## Phase 5: User Story 2 — GitHub Release Page Quality (Priority: P1)

**Goal**: A first-time visitor to the GitHub Releases page immediately understands what to download — labelled artifacts, readable changelog, correct version tag, stable vs. pre-release clearly distinguished.

**Independent Test**: Inspect a published test release page. Confirm: version tag matches Cargo version, changelog shows grouped conventional-commit entries (Features / Bug Fixes), artifact filenames identify platform and format, `checksums.txt` is present, and `latest` badge only appears on a stable tag.

- [x] T019 [P] [US2] Update `cliff.toml` to set a `[changelog]` `header` with the release tag and date template, and configure `[git] conventional_commits = true` with `filter_unconventional = true` so only conventional commit messages appear in the changelog
- [x] T020 [P] [US2] Add artifact upload labels in `release.yml` `publish` job: rename artifacts before upload so filenames include platform hint (e.g. `adagio_${VERSION}_linux_amd64.deb`, `adagio_${VERSION}_macos.dmg`) matching the Artifact Contract in `contracts/release-workflow.md`
- [x] T021 [US2] Add a `CONTRIBUTING.md` section (or update existing docs) documenting the release process: how to bump the version in `Cargo.toml`, push a tag, handle a botched release (delete draft + tag), and the pre-release suffix convention

**Checkpoint**: A test release page shows human-readable grouped changelog, all artifacts have platform-descriptive names, and a contributor can follow the documented steps to cut a release unaided.

---

## Phase 6: User Story 4 — System Integration on Linux (Priority: P2)

**Goal**: After `.deb`/`.rpm` install, Adagio appears in the GNOME/KDE application menu with the correct icon at all sizes, categorised correctly, and the autostart mechanism works.

**Independent Test**: On Ubuntu 22.04 GNOME: (a) `desktop-file-validate /usr/share/applications/adagio.desktop` passes with no errors; (b) `ls /usr/share/icons/hicolor/*/apps/adagio.png` lists all 8 sizes; (c) opening the app grid shows Adagio under the Network or Internet category with a sharp icon.

- [x] T022 [P] [US4] Add a CI validation step in `build-linux` after `.deb` install: run `desktop-file-validate /usr/share/applications/adagio.desktop` and fail the job if validation reports any errors; assert all required fields (Name, GenericName, Exec, Icon, Comment, Categories, Terminal) are present
- [x] T023 [P] [US4] Add an icon-presence check in `build-linux` CI: after `.deb` install, verify `ls /usr/share/icons/hicolor/{16x16,24x24,32x32,48x48,64x64,128x128,256x256,512x512}/apps/adagio.png` returns 8 files; fail if any are missing

**Checkpoint**: `desktop-file-validate` passes and all 8 icon sizes are present in the installed package.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [x] T024 [P] Add `docs/adr/017-release-packaging.md` recording the decision to use Tauri bundler + externalBin instead of separate packages or fpm, with rationale from `research.md` Finding 1 and Finding 7
- [x] T025 [P] Add Gatekeeper / notarization note to the macOS `.dmg` release description (via `cliff.toml` footer or release body template): "This binary is not notarized. macOS may show a security warning — right-click and choose Open to proceed."
- [x] T026 Run `cargo clippy -- -D warnings` and `cargo fmt --check` locally to confirm no regressions from tauri.conf.json or Rust-adjacent changes; fix any issues

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — blocks all user story phases
- **Phase 3 (US3 — Pipeline)**: Depends on Phase 2 — the release.yml needs the updated tauri.conf.json
- **Phase 4 (US1 — Install)**: Depends on Phase 3 (release.yml must produce packages before smoke tests can be added)
- **Phase 5 (US2 — Release page)**: Depends on Phase 3; cliff.toml (T007) must be complete
- **Phase 6 (US4 — Integration)**: Depends on Phase 4 (`.deb` install must work before integration checks)
- **Phase 7 (Polish)**: Depends on all prior phases

### Within-Phase Parallel Opportunities

- T003, T004, T005 can all run in parallel (separate sections of `tauri.conf.json`)
- T006, T007 can run in parallel (separate files)
- T011, T012 (build-linux, build-macos jobs) are parallel in CI
- T022, T023 can run in parallel (separate validation steps)
- T024, T025 can run in parallel (separate files)

---

## Parallel Example: Phase 2

```bash
# All foundational config tasks can be tackled simultaneously:
Task T003: Update tauri.conf.json icon array
Task T006: Create bundler/adagio.desktop.template
Task T007: Create cliff.toml
# Then T004 + T005 to finish tauri.conf.json
```

---

## Implementation Strategy

### MVP: US3 Only (Working Release Pipeline)

1. Complete Phase 1 (Setup)
2. Complete Phase 2 (Foundational config)
3. Complete Phase 3 (T008–T015) — release.yml end-to-end
4. **STOP and VALIDATE**: Push `v0.1.0-rc1` tag to a test branch; confirm release page appears with all artifacts
5. Ship if acceptable

### Full Delivery (All Stories)

1. Phases 1–3 (pipeline working)
2. Phase 4 (package installability smoke tests)
3. Phase 5 (release page quality)
4. Phase 6 (desktop integration validation)
5. Phase 7 (polish + ADR)

---

## Notes

- [P] tasks touch different files and have no shared in-progress dependencies
- Every CI step added to `release.yml` should also be documented in the "Test" section of each job with a meaningful `name:` so failures are readable in the GitHub Actions UI
- The `binaries/` directory holding daemon/CLI for Tauri `externalBin` should be `.gitignore`'d — it's populated at build time
- Tauri's `externalBin` requires the binary to carry a platform suffix at runtime (e.g. `adagio-daemon-x86_64-unknown-linux-gnu`); the `cargo tauri build` command handles this automatically when the binary is in `binaries/`
