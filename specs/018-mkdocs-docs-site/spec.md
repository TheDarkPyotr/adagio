# Feature Specification: Fumadocs Documentation Site

**Feature Branch**: `018-mkdocs-docs-site`

**Created**: 2026-06-04 | **Revised**: 2026-06-05

**Status**: Draft

---

## User Stories

### US1 — Browse and Read Documentation Online

A developer visits the Adagio documentation site hosted on GitHub Pages to understand how to install, configure, and use Adagio. They navigate the sidebar, read multi-section pages, and use the search bar to find topics.

**Independent Test**: Open the GitHub Pages URL; navigate Installation → Configuration → CLI Reference without a broken link; read a page with a fenced code block.

### US2 — Find a CLI Subcommand or IPC Message Type

A power user needs to check the exact flags for `adagio sync` and the JSON schema for `DaemonRequest::SyncNow`. They use the sidebar or search to locate the reference in under 30 seconds.

**Independent Test**: Search for "adagio sync"; confirm matching results render within 500 ms. Navigate Daemon IPC → Message Types and confirm `DaemonRequest` variants are listed.

### US3 — Automated Deployment on Every Merge

Every qualifying push to `main` automatically rebuilds and deploys the site to GitHub Pages with no human intervention; the live site reflects the change within 5 minutes.

**Independent Test**: Merge a PR that changes one `.mdx` file; confirm the GitHub Actions workflow completes successfully and the change appears on the live site.

### US4 — Contributor Adds a New Doc Page

A new contributor can build and preview the docs locally in under 5 minutes and add a new page without breaking anything.

**Independent Test**: Follow the Contributing → Build guide from a clean checkout; run the local dev server; add a new page; confirm it appears in the sidebar and the build passes with zero errors.

---

## Acceptance Scenarios

1. CLI Reference section exists; user searches for "adagio sync"; matching subcommand result is returned with flags listed.
2. IPC developer navigates to the `DaemonRequest` section; all variants from `crates/adagio-ipc/src/` are documented.
3. A PR that adds a new page is reviewed; all internal links resolve and no broken references are reported.

---

## Edge Cases

- What if a nav entry references an MDX file that does not exist? → `next build` must fail with a clear error, not silently produce a broken page.
- What if the GitHub Pages branch (`gh-pages`) is behind? → The deployment step must force-push to stay in sync.
- What if a page has no headings? → Search must still index it by filename and first paragraph.
- What if custom CSS references an unavailable font CDN? → Fonts must be self-hosted or bundled so the site renders without external network access.

---

## Content Sections (FR-007)

The site must include the following top-level sections in the sidebar:

- **Getting Started** — Installation, First Run, Account Setup
- **User Guide** — File Sync, Conflict Resolution, VFS on-demand, Bandwidth Controls, Network Awareness, E2EE Setup
- **CLI Reference** — All `adagio` subcommands with flags, examples, exit codes
- **Daemon & IPC** — IPC protocol reference, `DaemonRequest`/`DaemonResponse` with JSON schemas
- **Architecture** — Crate overview, sync engine internals, FUSE3 driver, E2EE protocol details
- **Contributing** — Build guide, development workflow, SpecKit overview
- **Changelog**

---

## Functional Requirements

- **FR-001**: Site is navigable via a persistent sidebar with collapsible sections.
- **FR-002**: Full-text search returns results within 500 ms of the last keystroke.
- **FR-003**: Site is responsive from 320 px to 2560 px.
- **FR-004**: A 404 page is rendered for unknown routes with midnight styling.
- **FR-005**: Every page shows an "Edit this page on GitHub" link pointing to the source MDX file.
- **FR-006**: Dark mode is locked to the Adagio midnight palette; no light/dark toggle is exposed.
- **FR-007**: Navigation tree covers all sections listed above (two levels of depth).
- **FR-008**: Syntax highlighting for Rust, TOML, JSON, Bash, and YAML.
- **FR-009**: Adagio logo/banner in the header.
- **FR-010**: "Edit this page" link visible on every content page.
- **FR-011**: Previous / Next page navigation at the bottom of each page.
- **FR-012**: Table of contents auto-generated from headings on each page.
- **FR-013**: External links open in a new tab.
- **FR-014**: Admonition components (Note, Warning, Danger, Tip) styled with midnight tokens.
- **FR-015**: Internal link resolution validated at build time; broken links fail the build.

---

## Success Criteria

- **SC-001**: Lighthouse Performance score ≥ 90 on the home page.
- **SC-002**: Search result latency ≤ 500 ms after last keystroke across the full corpus.
- **SC-003**: 100% of CLI subcommands and all IPC message types documented.
- **SC-004**: Site scores ≥ 90 on Lighthouse Accessibility.
- **SC-005**: A fresh contributor can build and preview documentation locally in under 5 minutes.
- **SC-006**: Changes merged to `main` are live on GitHub Pages within 5 minutes of merge.
- **SC-007**: Midnight colour scheme matches Adagio app pixel-for-pixel on background, text, and accent colours when compared side-by-side.
- **SC-008**: All internal links resolve with zero 404s verified at build time.

---

## Assumptions

- GitHub repository has GitHub Pages enabled (repo admin rights available).
- Geist and Geist Mono fonts are available under licence in WOFF2 format.
- No doc-comments (`rustdoc`) from `crates/` are pulled into the site — content lives in `content/docs/` MDX files only.
- Existing Markdown content under `docs/` will be migrated to MDX and moved into the new `docs-site/` app directory.
- Existing `mkdocs.yml` and `requirements-docs.txt` at the repo root will be removed.
