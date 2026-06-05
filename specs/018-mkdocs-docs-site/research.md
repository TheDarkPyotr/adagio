# Research: Fumadocs Documentation Site

**Feature**: 018-mkdocs-docs-site | **Date**: 2026-06-05

---

## R-001: Documentation Framework Choice

**Decision**: Fumadocs (Next.js App Router + `fumadocs-ui` + MDX)

**Rationale**:
- Fumadocs is a TypeScript-first documentation framework built on Next.js 15 App Router and MDX; it aligns with the adagio-desktop frontend stack (React/TypeScript).
- `fumadocs-ui` ships a polished default theme with full Tailwind CSS v4 CSS-variable theming — easy to override with Adagio midnight tokens.
- Built-in full-text search via Orama (no Algolia dependency); works entirely in the browser with no server required.
- Static export (`output: 'export'` in `next.config.mjs`) produces a plain HTML/CSS/JS bundle deployable to GitHub Pages without any server runtime.
- MDX allows embedding React components inside Markdown for admonitions, code blocks, and interactive demos.
- Package `fumadocs-core` + `fumadocs-ui` + `fumadocs-mdx` covers all FR-001–FR-015 requirements.

**Alternatives rejected**:
- *MkDocs + Material (Python)*: Strong GitHub Pages story but Python toolchain is inconsistent with the Node.js frontend. CSS variable override system is less expressive than Tailwind. Replacing previous decision.
- *Docusaurus 3*: React/MDX but heavier (Facebook internal tooling assumptions, more complex config). Less expressive theming than fumadocs.
- *VitePress*: Vue-based; midnight theme customisation more involved than Tailwind CSS variables.
- *mdBook*: Rust-native but limited component extensibility; no MDX.

---

## R-002: Adagio Midnight Theme — Tailwind CSS Variable Mapping

**Decision**: Override `fumadocs-ui`'s default CSS variables inside the `docs-site/` Tailwind config and `globals.css`.

`fumadocs-ui` uses a standard set of CSS custom properties inside `[data-theme="dark"]`. Map Adagio midnight tokens onto these variables:

| Adagio token | Hex value | fumadocs variable |
|---|---|---|
| `--cream` | `#0f1117` | `--background` |
| `--paper` | `#131820` | `--card` / `--popover` |
| `--cream-3` | `#1e2433` | `--muted` |
| `--sand` | `#2a3347` | `--border` / `--input` |
| `--ink` | `#e8eef8` | `--foreground` |
| `--ink-soft` | `#9aa5bb` | `--muted-foreground` |
| `--clay` | `#5b9bd6` | `--primary` |
| `--clay-soft` | `#82b4e0` | `--ring` |
| `--good` | `#6ab88a` | admonition tip / success |
| `--warn` | `#d4a04a` | admonition warning |
| `--danger` | `#d96650` | admonition danger |

The midnight scheme is hardcoded as the only theme; `fumadocs-ui`'s built-in theme toggle is disabled.

**Alternatives rejected**:
- Dark mode via `prefers-color-scheme` media query only: would still show a flash of wrong colours before JS runs. Hard-coding `data-theme="dark"` on `<html>` avoids this.
- Forking fumadocs-ui theme: unnecessary maintenance burden.

---

## R-003: Font Strategy

**Decision**: Self-host Geist and Geist Mono; load via `next/font/local` with `display: swap`; disable any CDN font loading.

**Rationale**:
- `next/font/local` handles WOFF2 loading, preloading, and subset optimisation automatically.
- Self-hosting eliminates the Google Fonts CDN dependency and aligns with SC-007 (midnight palette self-sufficiency).
- Geist (Vercel, SIL OFL 1.1) is freely redistributable.
- Font files live at `docs-site/public/fonts/` and are referenced via `next/font/local` in `docs-site/app/layout.tsx`.

---

## R-004: GitHub Actions Deployment

**Decision**: `next build` → `next export` → `peaceiris/actions-gh-pages` to push the `out/` directory to the `gh-pages` branch.

**Workflow trigger**: `push` to `main` filtered to `docs-site/**` and `.github/workflows/docs.yml` paths — avoids unnecessary deploys from Rust code changes.

**Steps**:
1. `actions/checkout` (full depth for any git-based metadata)
2. `actions/setup-node` with Node.js 22 LTS
3. `actions/cache` on `docs-site/node_modules` keyed by `docs-site/package-lock.json`
4. `npm ci` inside `docs-site/`
5. `npm run build` (runs `next build`)
6. `peaceiris/actions-gh-pages` — publishes `docs-site/out/` to `gh-pages`

**Alternatives rejected**:
- Vercel deployment: introduces an external dependency; GitHub Pages is zero-config for open-source repos.
- `JamesIves/github-pages-deploy-action`: equivalent to `peaceiris`; either works.

---

## R-005: Build-Time Link Validation

**Decision**: `fumadocs-mdx` validates all internal `[[...]]`-style links and standard `[text](path)` links at build time; broken links fail `next build`.

Additionally, a custom `remark` plugin (`remark-validate-links`) is added to catch any unresolved relative links that fumadocs-mdx misses.

**Alternatives rejected**:
- External link checker (`htmltest`): external URLs change independently and produce false positives; out of scope.
- Lychee link checker in CI: adds a separate job; fumadocs build-time validation is sufficient for internal links.

---

## R-006: Search

**Decision**: fumadocs built-in search powered by Orama (default fumadocs-ui integration).

**Rationale**:
- Zero configuration; search index is generated at build time and served as a static JSON file.
- Client-side search with sub-100 ms latency for the expected corpus size (~40 pages).
- No API key, no external service dependency.
- Search modal opens with `Ctrl+K` / `Cmd+K`; SC-002 (≤ 500 ms) is comfortably met.

**Alternatives rejected**:
- Algolia DocSearch: free for open source but requires application approval and external dependency.
- Pagefind: another static search option; Orama ships natively with fumadocs-ui.

---

## R-007: Content Directory Layout

**Decision**: Docs content lives in `docs-site/content/docs/`; fumadocs `source.config.ts` points `defineCollections` at that directory.

Each section has a `meta.json` for sidebar ordering:
```json
{
  "title": "Getting Started",
  "pages": ["installation", "first-run", "account-setup"]
}
```

Pages are `.mdx` files with YAML frontmatter:
```yaml
---
title: Installation
description: How to install Adagio on Linux.
---
```

Existing Markdown files from `docs/` are migrated to MDX in `docs-site/content/docs/` with `.md` → `.mdx` extension and no other content changes required (MDX is a strict superset of Markdown).

**Alternatives rejected**:
- Keeping content at repo-root `docs/`: fumadocs expects content under its app directory; mixing would complicate the `source.config.ts` path resolution.

---

## All NEEDS CLARIFICATION resolved

No NEEDS CLARIFICATION markers present.
