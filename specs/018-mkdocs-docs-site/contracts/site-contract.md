# Contract: Documentation Site

**Feature**: 018-mkdocs-docs-site

This contract defines the observable guarantees the documentation site must uphold. It is verified by `next build` in CI and by manual Lighthouse audits at release.

---

## Build Contract

| Guarantee | Mechanism | Failure action |
|-----------|-----------|---------------|
| All internal links resolve | `fumadocs-mdx` build-time validation | Build fails; deploy blocked |
| All `meta.json` page slugs have corresponding `.mdx` files | `next build` | Build fails |
| Site builds with zero TypeScript errors | `tsc --noEmit` in CI | Build fails |
| Built output lives in `docs-site/out/` | `next.config.mjs output: 'export'` | N/A |

---

## Deployment Contract

| Guarantee | Mechanism |
|-----------|-----------|
| `gh-pages` branch updated on every qualifying push to `main` | GitHub Actions workflow |
| Live site reflects latest `main` within 5 minutes of merge | Workflow SLA |
| Deployment never replaces live site with a broken build | `next build` must succeed before deploy step |
| Deployment is idempotent | `peaceiris/actions-gh-pages` force-push |

---

## Visual Contract

The following design properties are verified by visual review on each release:

| Property | Expected value | Source of truth |
|----------|---------------|-----------------|
| Page background | `#0f1117` | `--background` → `tokens.css` `--cream` midnight |
| Surface (sidebar, code) | `#131820` | `--card` → `tokens.css` `--paper` midnight |
| Border / hairline | `#2a3347` | `--border` → `tokens.css` `--sand` midnight |
| Primary text | `#e8eef8` | `--foreground` → `tokens.css` `--ink` midnight |
| Muted text | `#9aa5bb` | `--muted-foreground` → `tokens.css` `--ink-soft` midnight |
| Accent (links, active) | `#5b9bd6` | `--primary` → `tokens.css` `--clay` midnight |
| Body font | Geist, system-ui fallback | `next/font/local` |
| Mono font | Geist Mono, ui-monospace fallback | `next/font/local` |

---

## Performance Contract

| Metric | Target | Tool |
|--------|--------|------|
| Lighthouse Performance | ≥ 90 | Lighthouse CI |
| Lighthouse Accessibility | ≥ 90 | Lighthouse CI |
| Home page TTI | ≤ 2 s on 50 Mbps | Lighthouse CI |
| Search result latency | ≤ 500 ms | Manual / browser DevTools |

---

## Content Coverage Contract

| Section | Minimum coverage |
|---------|-----------------|
| CLI Reference | All `adagio` subcommands in `crates/adagio-cli/src/` |
| Daemon IPC | All `DaemonRequest` + `DaemonResponse` variants in `crates/adagio-ipc/src/` |
| Architecture | All 8 crates described |
| Getting Started | Install paths: DEB, RPM, AppImage, source |
| User Guide | All 6 feature areas: sync, conflict, VFS, bandwidth, network, E2EE |

---

## Accessibility Contract

All text elements must meet WCAG 2.1 AA:
- Normal text contrast ratio ≥ 4.5:1 against background
- Large text contrast ratio ≥ 3:1
- Interactive elements have visible focus indicators
- All images have `alt` attributes
