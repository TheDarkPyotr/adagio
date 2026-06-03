# Adagio · Design Handoff

Drop this folder into your repo (suggested: `docs/design/` or `handoff/`).
Then point Claude Code at it.

## What's here

```
handoff/
├── DESIGN.md            ← read this first — the design system & spec
├── README.md            ← this file
├── tokens.json          ← all 8 palettes + type scale as JSON
├── prototype.html       ← open in a browser; interactive design canvas
├── screenshots/         ← reference PNGs for every screen
│   ├── 01-onboard.png
│   ├── 02-files.png
│   ├── 03-activity.png
│   ├── 04-share.png
│   └── 05-tray.png
└── source/              ← the React prototype source
    ├── tokens.css         ← drop straight into your global stylesheet
    ├── logos.jsx          ← the mark + three logo directions
    ├── desktop-app.jsx    ← chrome, sidebar, icons, sample data
    ├── desktop-scenes.jsx ← files / onboard / activity / share / tray
    ├── website.jsx        ← marketing site (tone reference)
    └── banners.jsx        ← README banners (tone reference)
```

## Recommended workflow with Claude Code

### 1. Add a `CLAUDE.md` at your repo root

This makes Claude read the design spec on every chat without you having to ask.
Add something like:

```md
# Adagio Desktop Client

## Design

All design decisions are documented in `handoff/DESIGN.md`. Read it before
making UI changes. Screenshots of the intended UI live in `handoff/screenshots/`.

## Tech

- Tauri 2 shell, React + TypeScript frontend, Rust sync engine (`cadence` crate).
- `cadence` has no Tauri dependencies — it's testable headlessly.
- All server I/O goes through `cadence`, exposed via Tauri `invoke()` commands.
- No telemetry. Ever. AGPL-3.0.

## Voice in copy

Quiet, direct, no marketing fluff, no emoji. Musical metaphors over computer
metaphors ("cadence" not "rate"). See handoff/DESIGN.md §2.
```

### 2. First prompt to Claude Code

```
Read handoff/DESIGN.md. Then read handoff/source/desktop-app.jsx and
handoff/source/desktop-scenes.jsx. Look at handoff/screenshots/02-files.png
and 03-activity.png. Confirm you understand the system before we build anything.
```

### 3. When working on a screen

Always reference the screenshot path so Claude can open it:

```
Implement the Files scene as a React component in src/scenes/Files.tsx.
Reference: handoff/screenshots/02-files.png and handoff/source/desktop-scenes.jsx
(the `FilesScene` function). Use Tauri invoke to fetch the file list from
the cadence::files command; for now stub it with the FILE_TREE sample data.
```

### 4. When changing tokens or palettes

Edit `handoff/tokens.json` — it's the source of truth. Then regenerate
`tokens.css` from it (Claude can do this with a one-liner script).

## If you also want to iterate on design

This handoff was generated from the project at
<https://anthropic.com/projects/...>. To explore new design directions, open
that project (or fork it) — the interactive design canvas there is the fastest
loop for visual changes. Then re-export to this folder.

## License

Brand & copy © Sound GmbH. Design system source is yours to use in the Adagio
desktop client. Fonts (Geist, Instrument Serif) are loaded from Google Fonts
or self-hostable — check their respective licenses.
