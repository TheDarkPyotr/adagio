# Implementation Plan: Fumadocs Documentation Site

**Branch**: `018-mkdocs-docs-site` | **Date**: 2026-06-05 | **Spec**: [spec.md](spec.md)

## Summary

Build a fully themed Fumadocs documentation site for Adagio using Next.js 15 App Router and MDX, covering every user-facing feature, the CLI reference, and the IPC protocol. Deploy it automatically to GitHub Pages on every push to `main`. The site applies Adagio's midnight design tokens via Tailwind CSS variable overrides so it looks like a natural extension of the desktop app. Existing Markdown content from `docs/` is migrated to MDX in `docs-site/content/docs/`; `mkdocs.yml` and `requirements-docs.txt` are removed.

## Technical Context

**Language/Version**: TypeScript 5.x; Node.js 22 LTS.

**Framework**: Next.js 15 (App Router), `output: 'export'` for static GitHub Pages deployment.

**Primary Dependencies**:
- `fumadocs-core` ≥ 14 — navigation, search index, content utilities
- `fumadocs-ui` ≥ 14 — default theme, layout components, search modal
- `fumadocs-mdx` ≥ 11 — MDX → source collection, frontmatter parsing, remark/rehype pipeline
- `next` ≥ 15 — App Router, `next/font/local`, static export
- `tailwindcss` ≥ 4 — utility CSS; midnight token overrides
- `@shikijs/rehype` (bundled with fumadocs-mdx) — syntax highlighting for Rust, TOML, JSON, Bash, YAML
- Self-hosted Geist + Geist Mono fonts via `next/font/local`

**Storage**: Static files only — `docs-site/` directory contains the Next.js app; `docs-site/content/docs/` contains MDX content. GitHub Pages hosts the built output (`docs-site/out/`) from the `gh-pages` branch.

**Testing**: `npm run build` inside `docs-site/` (runs `next build`) must succeed with zero errors. Broken internal links fail the build via fumadocs-mdx validation.

**Target Platform**: GitHub Pages (any modern browser); responsive from 320 px to 2560 px.

**Project Type**: Static documentation site (Next.js static export).

**Performance Goals**: Lighthouse Performance ≥ 90; page load ≤ 2 s on 50 Mbps; search results ≤ 500 ms.

**Constraints**: No server runtime (static export only); all fonts self-hosted; build must complete in ≤ 3 minutes on GitHub-hosted runners; midnight scheme locked — no light/dark toggle.

**Scale/Scope**: ~30–40 MDX pages across 7 top-level sections; ~800 kB of built HTML/CSS/JS.

## Constitution Check

| Gate | Principle | Status |
|------|-----------|--------|
| Tests authored and FAIL before implementation begins | I. Test-First | N/A — static site; `next build` is the gating check |
| All public Rust items have `///` doc comments | II. Documentation as Code | N/A — no Rust code in this feature |
| ADR recorded in `docs/adr/` for significant design decisions | II. Documentation as Code | ✅ ADR-018 updated to document Fumadocs choice |
| Structured logging added to all new sync/network operations | III. Observability | N/A — no runtime code |
| No `println!` in production code paths | III. Observability | N/A |
| New feature implemented as independent crate/module | IV. Extensibility | N/A — docs toolchain separate from Rust workspace |
| Cross-module calls go through defined trait/interface contracts | IV. Extensibility | N/A |
| Idle memory budget < 100 MB RSS | V. Performance-Oriented | N/A — static site |
| UI actions provide feedback within 100 ms | V. Performance-Oriented | N/A |
| `cargo clippy -- -D warnings` passes | Dev Workflow | N/A — no Rust code |
| `cargo fmt --check` passes | Dev Workflow | N/A |

**Constitution verdict**: All applicable gates pass. This feature sits entirely outside the Rust workspace. The one applicable gate (Documentation as Code — ADR) is satisfied by updating ADR-018.

## Project Structure

```text
adagio/                                   # repo root
├── docs-site/                            # Next.js + fumadocs app
│   ├── app/
│   │   ├── layout.tsx                    # Root layout (Geist font, <html data-theme="dark">)
│   │   ├── page.tsx                      # Home page (redirects to /docs or landing)
│   │   ├── globals.css                   # Tailwind base + midnight CSS variable overrides
│   │   └── docs/
│   │       ├── layout.tsx                # fumadocs DocsLayout with sidebar
│   │       └── [[...slug]]/
│   │           └── page.tsx              # Dynamic MDX page renderer
│   ├── content/
│   │   └── docs/                         # All MDX content
│   │       ├── index.mdx                 # Home / overview
│   │       ├── getting-started/
│   │       │   ├── meta.json
│   │       │   ├── installation.mdx
│   │       │   ├── first-run.mdx
│   │       │   └── account-setup.mdx
│   │       ├── user-guide/
│   │       │   ├── meta.json
│   │       │   ├── file-sync.mdx
│   │       │   ├── conflict-resolution.mdx
│   │       │   ├── vfs.mdx
│   │       │   ├── bandwidth.mdx
│   │       │   ├── network-awareness.mdx
│   │       │   └── e2ee.mdx
│   │       ├── cli-reference/
│   │       │   ├── meta.json
│   │       │   └── index.mdx
│   │       ├── daemon-ipc/
│   │       │   ├── meta.json
│   │       │   ├── protocol.mdx
│   │       │   └── message-types.mdx
│   │       ├── architecture/
│   │       │   ├── meta.json
│   │       │   ├── overview.mdx
│   │       │   ├── sync-engine.mdx
│   │       │   ├── vfs-driver.mdx
│   │       │   └── e2ee-protocol.mdx
│   │       ├── contributing/
│   │       │   ├── meta.json
│   │       │   ├── build.mdx
│   │       │   ├── dev-workflow.mdx
│   │       │   └── speckit.mdx
│   │       └── changelog.mdx
│   ├── public/
│   │   ├── fonts/                        # Self-hosted Geist + Geist Mono WOFF2
│   │   └── adagio_banner.png
│   ├── source.config.ts                  # fumadocs-mdx defineCollections config
│   ├── next.config.mjs                   # output: 'export', basePath if needed
│   ├── tailwind.config.ts                # midnight token overrides
│   ├── package.json
│   └── tsconfig.json
├── docs/adr/
│   └── ADR-018-fumadocs-midnight.md      # Updated ADR (replaces mkdocs version)
├── .github/
│   └── workflows/
│       └── docs.yml                      # Build + deploy to GitHub Pages
│
│   [Removed from repo root:]
│   mkdocs.yml                            # DELETED
│   requirements-docs.txt                 # DELETED
│   docs/stylesheets/                     # DELETED (content migrated to docs-site/)
│   docs/javascripts/                     # DELETED
│   docs/overrides/                       # DELETED
```

## Phase 0: Research

See [research.md](research.md).

## Phase 1: Design & Contracts

See [data-model.md](data-model.md) and [contracts/](contracts/).
