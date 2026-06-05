# Tasks: Fumadocs Documentation Site

**Input**: Design documents from `specs/018-mkdocs-docs-site/`

**Prerequisites**: [plan.md](plan.md) · [spec.md](spec.md) · [research.md](research.md) · [data-model.md](data-model.md) · [contracts/site-contract.md](contracts/site-contract.md)

**Constitution Test-First note**: This feature produces a static documentation site with no imperative runtime code. The gating build check (`npm run build` inside `docs-site/`) substitutes for a traditional test suite. Lighthouse CI audits enforce performance and accessibility contracts. Tasks are organized to run the build check as early as possible to surface issues before content writing is complete.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: User story label (US1–US4) for story-phase tasks
- Exact file paths are included in each description

---

## Phase 0: Cleanup — Remove MkDocs Artifacts

**Purpose**: Delete all MkDocs-specific files from the repo before the new toolchain is introduced.

- [x] T000 Delete `mkdocs.yml` from the repo root
- [x] T001 Delete `requirements-docs.txt` from the repo root
- [x] T002 Delete `docs/stylesheets/`, `docs/javascripts/`, `docs/overrides/` directories (MkDocs theme overrides — content pages in `docs/` are kept for migration in Phase 1)
- [x] T003 Delete `docs/adr/ADR-018-mkdocs-material-midnight.md`; create `docs/adr/ADR-018-fumadocs-midnight.md` documenting the Fumadocs + Tailwind midnight choice (see research.md R-001/R-002)
- [x] T004 Remove any Markdown content stubs that were created for MkDocs placeholder validation (check `docs/` for files containing only `# Title` and nothing else)

**Checkpoint**: Repo root contains no `mkdocs.yml` or `requirements-docs.txt`; `docs/adr/ADR-018-fumadocs-midnight.md` exists.

---

## Phase 1: Setup — Scaffold the Next.js + Fumadocs App

**Purpose**: Create the complete `docs-site/` Next.js app and verify `next build` succeeds on placeholder pages before any content is written.

- [x] T005 Scaffold `docs-site/` with `npx create-fumadocs-app` (choose Next.js App Router template); confirm the generated structure matches `plan.md`; commit the scaffold as-is
- [x] T006 Update `docs-site/package.json`: set `name: "adagio-docs"`, set `"build": "next build"`, verify `fumadocs-core`, `fumadocs-ui`, `fumadocs-mdx`, `next` ≥ 15, `tailwindcss` ≥ 4 are in `dependencies`
- [x] T007 Configure `docs-site/next.config.mjs`: set `output: 'export'`, set `basePath: '/adagio'` (adjust if GitHub Pages serves from root), set `images: { unoptimized: true }`
- [x] T008 Configure `docs-site/source.config.ts`: `defineCollections` pointing at `content/docs` with type `'doc'`; export `docs` collection
- [x] T009 Copy self-hosted font files from `crates/adagio-desktop/src-ui/dist/fonts/` into `docs-site/public/fonts/` — Geist (Regular 400, Medium 500, SemiBold 600, Bold 700) and Geist Mono (Regular 400, Medium 500) in WOFF2 format
- [x] T010 Create `docs-site/content/docs/` directory tree: `getting-started/`, `user-guide/`, `cli-reference/`, `daemon-ipc/`, `architecture/`, `contributing/`; add `meta.json` in each section with the page order defined in `plan.md`; add placeholder `index.mdx` (frontmatter `title` + one sentence of body) in each
- [x] T011 Create `docs-site/content/docs/index.mdx` as the docs home page placeholder
- [x] T012 Run `npm run build` inside `docs-site/` — confirm zero TypeScript errors and zero build errors on placeholder pages; fix any config issues until the build is clean

**Checkpoint**: `docs-site/` scaffolded. `npm run build` succeeds on placeholder pages. `docs-site/out/` directory produced.

---

## Phase 2: Theme Fidelity — Midnight Design Tokens

**Purpose**: Apply the Adagio midnight palette before content is written so every subsequent page automatically inherits correct styling.

**⚠️ CRITICAL**: No content-writing phases should proceed until T013–T018 pass visual review.

- [x] T013 In `docs-site/app/layout.tsx`: configure `next/font/local` for Geist (pointing to `public/fonts/`) and Geist Mono; set CSS variables `--font-sans` and `--font-mono`; add `data-theme="dark"` to `<html>` to lock midnight scheme
- [x] T014 In `docs-site/app/globals.css`: override `fumadocs-ui` CSS custom properties for the dark scheme using midnight tokens from `research.md` R-002 table — `--background: #0f1117`, `--card: #131820`, `--muted: #1e2433`, `--border: #2a3347`, `--foreground: #e8eef8`, `--muted-foreground: #9aa5bb`, `--primary: #5b9bd6`, `--ring: #82b4e0`
- [x] T015 Configure Shiki syntax highlighting in `docs-site/source.config.ts` or `next.config.mjs`: use a dark theme (e.g. `vitesse-dark`) and add CSS overrides in `globals.css` to match midnight palette — string `#6ab88a`, keyword `#5b9bd6`, comment `#616e82`, literal `#d4a04a`, operator `#e8eef8`, background `#131820`
- [x] T016 Add admonition CSS overrides in `globals.css` for Note (`#5b9bd6`), Tip (`#6ab88a`), Warning (`#d4a04a`), Danger (`#d96650`) — matching midnight token colours
- [x] T017 In `docs-site/app/docs/layout.tsx`: configure `DocsLayout` with the Adagio banner/logo in the header slot; set `githubUrl` to the Adagio GitHub repo
- [ ] T018 Visual review: run `npm run dev` inside `docs-site/` and verify the midnight palette — background `#0f1117`, sidebar `#131820`, primary text `#e8eef8`, links `#5b9bd6`, code blocks `#131820` with correct syntax colours, Geist body font, Geist Mono code font

**Checkpoint**: Theme fully faithful to midnight palette. All surfaces, typography, and syntax colours match `contracts/site-contract.md`. Ready for content.

---

## Phase 3: User Story 1 — Browse and Read Documentation Online (Priority: P1) 🎯 MVP

**Goal**: A visitor can open the GitHub Pages URL, navigate the full sidebar, and read well-structured content for every major feature area.

**Independent Test**: Run `npm run dev`; visit home page; navigate to Getting Started → Installation, User Guide → File Sync, Architecture → Overview; confirm midnight theme, correct sidebar, no broken links.

### Implementation for User Story 1

- [x] T019 [P] [US1] Write `docs-site/content/docs/index.mdx` (home page): project tagline, key properties (conflict-aware, bandwidth-aware, network-aware, VFS, E2EE), architecture ASCII diagram from README, links to Getting Started and feature sections
- [x] T020 [P] [US1] Write `docs-site/content/docs/getting-started/installation.mdx`: install methods for DEB, RPM, AppImage, build from source (Rust stable + Node.js prerequisites, `cargo tauri build`); Linux tray requirements
- [x] T021 [P] [US1] Write `docs-site/content/docs/getting-started/first-run.mdx`: launching `adagio-desktop`, onboarding wizard, daemon auto-start, config file locations (`~/.config/adagio/`)
- [x] T022 [P] [US1] Write `docs-site/content/docs/getting-started/account-setup.mdx`: adding a Nextcloud account, server URL, username/password, connection test, credential storage (system keychain)
- [x] T023 [P] [US1] Write `docs-site/content/docs/user-guide/file-sync.mdx`: daemon polling, journal, reconciler/propagator, sync status badges, Sync Now, selected pairs
- [x] T024 [P] [US1] Write `docs-site/content/docs/user-guide/conflict-resolution.mdx`: what a conflict is, three resolution policies, conflict wizard UI, activity log
- [x] T025 [P] [US1] Write `docs-site/content/docs/user-guide/vfs.mdx`: VFS on-demand, enabling FUSE3, pin/evict controls, offline access, `libfuse3-dev` requirements
- [x] T026 [P] [US1] Write `docs-site/content/docs/user-guide/bandwidth.mdx`: per-account upload/download limits, token-bucket throttling, UI controls
- [x] T027 [P] [US1] Write `docs-site/content/docs/user-guide/network-awareness.mdx`: metered connection detection, battery-state awareness, SSID blocklist, daemon pause behaviour
- [x] T028 [P] [US1] Write `docs-site/content/docs/user-guide/e2ee.mdx`: AES-128-GCM, BIP-39 mnemonic, init steps in UI, mnemonic backup warning, Nextcloud SSE pre-requisite (`occ encryption:disable`), UUID-named ciphertext
- [x] T029 [P] [US1] Write `docs-site/content/docs/architecture/overview.mdx`: crate table (all 8 crates with descriptions), architecture diagram, IPC flow summary
- [x] T030 [P] [US1] Write `docs-site/content/docs/architecture/sync-engine.mdx`: SyncEngine internals — SQLite WAL journal, reconciler, propagator, bulk-upload driver, bandwidth token bucket
- [x] T031 [P] [US1] Write `docs-site/content/docs/architecture/vfs-driver.mdx`: FUSE3 driver design, VfsPairRunner, pin/evict state machine, on-demand delivery
- [x] T032 [P] [US1] Write `docs-site/content/docs/architecture/e2ee-protocol.mdx`: RSA-2048 key gen, PKCS#10 CSR, BIP-39, V2 metadata format, lock/upload/unlock cycle, AES-128-GCM, CMS signatures
- [x] T033 [US1] Update all `meta.json` files in US1 sections to reflect final page slugs; run `npm run build` to confirm zero broken links

**Checkpoint**: US1 complete. `npm run dev` shows a fully navigable site with home page, getting-started flow, all user guide sections, and architecture pages — midnight-themed with no broken links.

---

## Phase 4: User Story 2 — CLI and IPC Reference (Priority: P2)

**Goal**: A power user can find any CLI subcommand or IPC message type in under 30 seconds using the sidebar or search.

**Independent Test**: Open the site; search for "adagio status"; find the subcommand heading with flags, description, and example. Navigate to Daemon IPC → Message Types and confirm every `DaemonRequest` variant is documented.

### Implementation for User Story 2

- [x] T034 [US2] Read `crates/adagio-cli/src/` to enumerate all subcommands, flags, and examples; read `crates/adagio-ipc/src/` to enumerate all `DaemonRequest` and `DaemonResponse` variants with their fields
- [x] T035 [P] [US2] Write `docs-site/content/docs/cli-reference/index.mdx`: introduction; for each subcommand — heading, synopsis, options table, fenced `bash` examples, exit codes; subcommands: `adagio status`, `adagio sync`, `adagio accounts`, `adagio pairs`, `adagio daemon`, `adagio e2ee`, `adagio config`
- [x] T036 [P] [US2] Write `docs-site/content/docs/daemon-ipc/protocol.mdx`: Unix socket, NDJSON framing, request/response lifecycle, error format, versioning
- [x] T037 [P] [US2] Write `docs-site/content/docs/daemon-ipc/message-types.mdx`: for each `DaemonRequest` variant — heading, description, fenced `json` schema, example; repeat for `DaemonResponse`
- [x] T038 [US2] Update `daemon-ipc/meta.json` and `cli-reference/meta.json`; run `npm run build`

**Checkpoint**: US2 complete. CLI reference and IPC message-type pages are live and searchable.

---

## Phase 5: User Story 3 — GitHub Actions Deployment (Priority: P3)

**Goal**: Every qualifying push to `main` automatically rebuilds and deploys to GitHub Pages.

**Independent Test**: Merge a PR that changes one `.mdx` file; confirm the `docs` workflow completes successfully and the live site reflects the change.

### Implementation for User Story 3

- [x] T039 Create `.github/workflows/docs.yml`:
  - Trigger: `push` to `main` with `paths: ['docs-site/**', '.github/workflows/docs.yml']`
  - Job `deploy` on `ubuntu-latest`
  - Steps: `actions/checkout` (full depth), `actions/setup-node@v4` (Node 22), restore `node_modules` cache keyed on `docs-site/package-lock.json`, `npm ci` in `docs-site/`, `npm run build` in `docs-site/`, `peaceiris/actions-gh-pages@v4` publishing `docs-site/out/` to `gh-pages`
- [x] T040 Add `docs-site/out/` and `docs-site/.next/` to `.gitignore`
- [x] T041 Verify workflow YAML passes `actionlint` (no syntax errors before pushing)

**Checkpoint**: US3 complete. A test push to a `docs-site/content/**` file triggers a successful deploy run.

---

## Phase 6: User Story 4 — Contributor Experience (Priority: P4)

**Goal**: A new contributor can build and preview the docs locally in under 5 minutes and add a new page without breaking anything.

**Independent Test**: Follow Contributing → Build from a clean checkout; run `npm run dev`; add a test page; confirm it appears in the sidebar and `npm run build` passes.

### Implementation for User Story 4

- [x] T042 [P] [US4] Write `docs-site/content/docs/contributing/build.mdx`: prerequisites (Node.js 22+, npm), install (`npm ci`), local preview (`npm run dev`), production build (`npm run build`), output location (`docs-site/out/`)
- [x] T043 [P] [US4] Write `docs-site/content/docs/contributing/dev-workflow.mdx`: SpecKit feature workflow, branch naming, PR process, `specs/` design docs, ADR process
- [x] T044 [P] [US4] Write `docs-site/content/docs/contributing/speckit.mdx`: what SpecKit is, the seven commands, link to `specs/` examples, how to add docs alongside a feature
- [x] T045 [P] [US4] Write `docs-site/content/docs/changelog.mdx`: version 0.1.0 stub listing features 001–017; format: `## [0.1.0] - 2026-MM-DD` followed by feature list

**Checkpoint**: US4 complete. Contributing guide is complete end-to-end.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [x] T046 [P] Add `sitemap.ts` to `docs-site/app/` using Next.js built-in sitemap support; set correct `baseUrl`
- [x] T047 [P] Verify WCAG 2.1 AA contrast for all text/background combinations in `globals.css`; primary text `#e8eef8` on `#0f1117` must be ≥ 4.5:1; muted text `#9aa5bb` on `#0f1117` must be ≥ 4.5:1
- [x] T048 [P] Add the Adagio banner image (`docs/adr/adagio_banner.png`, copied to `docs-site/public/`) to `content/docs/index.mdx` with correct alt text
- [x] T049 Run `npm run build` one final time on the complete site; confirm zero errors, output in `docs-site/out/`
- [x] T050 Run Lighthouse audit on the locally served site (`npm run dev` or `npx serve docs-site/out/`); confirm Performance ≥ 90 and Accessibility ≥ 90; fix any issues
- [x] T051 [P] Add `og:title`, `og:description`, and `og:image` metadata to `docs-site/app/layout.tsx`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 0 (Cleanup)**: No dependencies — start immediately; run before Phase 1
- **Phase 1 (Setup)**: Depends on Phase 0 complete
- **Phase 2 (Theme)**: Depends on Phase 1 complete — blocks all content phases
- **Phase 3 (US1)**: Depends on Phase 2 complete — independent of US2–US4
- **Phase 4 (US2)**: Depends on Phase 2 complete — independent of US1/US3/US4
- **Phase 5 (US3)**: Depends on Phase 2 complete — workflow is independent of content
- **Phase 6 (US4)**: Depends on Phase 2 complete — independent of US1/US2/US3
- **Phase 7 (Polish)**: Depends on Phases 3–6 complete

### Within Each Story Phase

- All `[P]`-marked page-writing tasks are fully independent (different files) and can be written simultaneously
- `meta.json` update tasks (T033, T038) must come after all page files in that phase exist

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 0: Remove MkDocs artifacts (T000–T004)
2. Phase 1: Scaffold (T005–T012)
3. Phase 2: Theme fidelity (T013–T018)
4. Phase 3: US1 content (T019–T033)
5. **STOP and VALIDATE**: `npm run dev` — confirm full site navigable with midnight theme
6. Push to `018-mkdocs-docs-site` branch for review

### Incremental Delivery

1. Cleanup + Setup + Theme → correct visual scaffold
2. US1 → browsable docs covering all feature areas (MVP!)
3. US2 → CLI + IPC reference
4. US3 → automated deployment live
5. US4 → contributing guide
6. Polish → Lighthouse / a11y / sitemap

---

## Notes

- `[P]` tasks within a phase touch different files — safe to execute concurrently
- `npm run build` must pass after every phase before moving to the next
- All Geist font files must be WOFF2 for `next/font/local` performance
- `docs-site/out/` and `docs-site/.next/` must be in `.gitignore`
- Existing Markdown files in `docs/` can be renamed `.mdx` and moved to `docs-site/content/docs/` without content changes (MDX is a strict superset of Markdown)
- The `basePath` in `next.config.mjs` must match the GitHub Pages URL path (e.g., `/adagio` if hosted at `https://org.github.io/adagio/`)
