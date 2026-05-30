# ADR 004: Design Token Strategy

**Date**: 2026-05-25
**Status**: Accepted
**Feature**: 004-ui-refactor-handoff

## Decision

Use CSS custom properties scoped to a `[data-palette]` HTML attribute selector on the `<html>` element to implement the eight-palette design token system. Palette switching is performed by the Svelte theme store changing `document.documentElement.dataset.palette` at runtime.

## Context

Adagio's UI needed a design-system refresh based on the `handoff/` design file. The system requires:
- Eight selectable color themes with a warm-earth default (`sienna`)
- One dark mode theme (`ink`), auto-detected from OS `prefers-color-scheme`
- Instant, no-reload palette switching
- Self-hosted typography (Geist, Geist Mono, Instrument Serif italic)
- Compliance with `default-src 'self'` CSP (no CDN font loads)

## Alternatives Considered

| Option | Rejected reason |
|--------|----------------|
| Tailwind CSS | Would require shipping the full utility class set; no benefit for a component-level design with only 16 semantic color roles |
| CSS Modules | Scoping per-component means palette-switch would need JS-driven class updates on every node; runtime cost is O(n components) |
| vanilla-extract | Build-time only; does not support runtime switching without re-bundling or PostCSS transforms |
| CSS-in-JS (e.g., Emotion) | Not available for Svelte 5 without adapter; adds JS runtime overhead |

## Implementation

- `src/lib/design/tokens.css`: 8 palette blocks as `[data-palette="<name>"] { --cream: …; … }`, plus shared typography and radius tokens on `:root`
- `src/lib/design/base.css`: `@font-face` declarations pointing to `/fonts/*.woff2`, global resets, focus-ring rules, `prefers-reduced-motion` override
- `src/lib/stores/theme.ts`: Svelte 5 `$state` store; reads palette from `get_palette` IPC on init; falls back to OS preference; writes `document.documentElement.dataset.palette` on every change
- Rust: `get_palette` / `set_palette` commands in `commands/prefs.rs` persist to `config.json`

## Consequences

- Palette switch is O(1): one attribute change on `<html>` triggers CSS cascade re-evaluation natively
- No JS runtime cost per-component for theming
- Fonts are self-hosted; new weights require adding `.woff2` files to `public/fonts/`
- Adding a ninth palette requires one CSS block in `tokens.css` + one Rust validation entry in `prefs.rs`
