# ADR-018: Fumadocs + Tailwind CSS Midnight Theme for Documentation Site

**Status**: Accepted
**Date**: 2026-06-05
**Supersedes**: ADR-018-mkdocs-material-midnight.md (MkDocs approach, rejected)

---

## Context

Adagio needs a public documentation site covering installation, user guide, CLI reference, IPC protocol, architecture, and contribution guide. The site must be hosted on GitHub Pages, support full-text search, apply the Adagio midnight colour palette, and be maintainable by the same contributors who work on the TypeScript/React desktop frontend.

An earlier draft specified MkDocs + Material theme (Python toolchain). That approach was rejected in favour of a TypeScript-native solution before any implementation began.

---

## Decision

Use **Fumadocs** (`fumadocs-core` + `fumadocs-ui` + `fumadocs-mdx`) on **Next.js 15 App Router** with a static export (`output: 'export'`) deployed to GitHub Pages.

Apply Adagio midnight design tokens via **Tailwind CSS v4** CSS custom property overrides in `globals.css`, hardcoding `data-theme="dark"` on `<html>` so the midnight scheme is always active.

---

## Rationale

| Concern | Choice | Why |
|---------|--------|-----|
| Language alignment | TypeScript | Matches adagio-desktop frontend; no second toolchain |
| Static export | `next export` → GitHub Pages | Zero server runtime; same hosting as before |
| Theming | Tailwind CSS v4 CSS vars | Direct mapping from `tokens.css` midnight palette; easy to maintain |
| Search | Orama (fumadocs built-in) | Zero config; client-side; no API key |
| Content | MDX | Superset of Markdown; allows React admonition components |
| Fonts | `next/font/local` | Automatic WOFF2 preloading and CSS variable injection |

---

## Midnight Token Mapping

| Adagio token | Hex | fumadocs-ui CSS var |
|---|---|---|
| `--cream` | `#0f1117` | `--background` |
| `--paper` | `#131820` | `--card` |
| `--cream-3` | `#1e2433` | `--muted` |
| `--sand` | `#2a3347` | `--border` |
| `--ink` | `#e8eef8` | `--foreground` |
| `--ink-soft` | `#9aa5bb` | `--muted-foreground` |
| `--clay` | `#5b9bd6` | `--primary` |
| `--clay-soft` | `#82b4e0` | `--ring` |

---

## Consequences

- `docs-site/` directory is a standalone Next.js app inside the monorepo; `npm ci && npm run build` produces `docs-site/out/` which is published to `gh-pages`.
- `mkdocs.yml` and `requirements-docs.txt` are removed from the repo root.
- Existing Markdown content from `docs/` is migrated to MDX under `docs-site/content/docs/`.
- The Python toolchain is no longer required for documentation.
