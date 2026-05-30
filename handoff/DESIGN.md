# Adagio · Design System & Spec

This is the single source of truth for the Adagio desktop client's design.
Read this first when working on the client UI.

Paired files:

- `screenshots/` — high-fidelity PNGs of every screen, captured from the React prototype
- `source/` — JSX prototype components that produced the screenshots (reference, not the implementation)
- `prototype.html` — open this in a browser to interact with the prototype
- `tokens.css` — drop-in CSS custom properties + eight palette themes
- `tokens.json` — same tokens as JSON for non-CSS consumers (Tauri config, etc.)

---

## 1 · The brand in one paragraph

Adagio is a desktop client for Nextcloud. It is **quiet, deliberate, out of your way**.
Where the official client is loud and dialog-heavy, Adagio sits between your file
manager and your server, keeps them in tempo, and only surfaces what's worth
your attention. The brand metaphor is musical (adagio = a slow, graceful tempo).
Aesthetic is **editorial & serene**: a sans-led type system with Instrument Serif
italic accents on key headlines, warm earth tones, generous whitespace, and a
hand-drawn slur mark (two notes tied by a curve) as the core symbol.

---

## 2 · Voice & tone

Confident, direct, product-led. Short sentences. No marketing fluff. No emoji.
A few examples of in-app copy that hit the mark:

- "Up to date" (not "Sync complete — All your files are synchronized")
- "A workspace, not a dashboard"
- "Begin softly" (the download CTA)
- "All systems quiet" (status indicator)
- "Files at a graceful tempo" (tagline)
- "Resolution without panic" (conflict resolution)

When in doubt, lean on musical and architectural metaphors before computer
metaphors. "Cadence" not "rate", "tempo" not "speed", "in sync" not "synchronized".

---

## 3 · Type system

Three families, used sparingly.

| Use | Family | Weight | Tracking |
|---|---|---|---|
| Body, UI, headlines (default) | Geist | 400 / 500 | −1% to −5.5% |
| Technical metadata, timestamps, paths, kbd | Geist Mono | 400 / 500 | +4% to +6% |
| Italic accent on key words ("in sync", "softly", "made of") | Instrument Serif italic | 400 | −1% |

**Rules:**

- **Headlines: Geist 500 with tight tracking** (−4.5% to −5.5% on display sizes). Mix in *one* serif italic phrase per headline as accent — not more.
- **Body: Geist 400** at 16px / 1.55 line-height. Lede paragraphs go up to 19px.
- **All metadata in mono.** Timestamps, paths, file sizes, byte counts, hashes, version strings. Mono signals "fact, not voice."
- **Never use the serif for body text.** It only ever appears as a 1–3-word accent inside a sans headline. The exception is the manifesto pull-quote which goes full serif.
- **No emoji.** Ever.

Sizes:

```
display-lg   96 / 0.95 / -0.05    "Begin softly."
display     100 / 0.98 / -0.055   the hero headline
display-sm   72 / 1.02 / -0.045   section titles
h1           56 / 1.02 / -0.05    in-app screen titles
h2           40 / 1.05 / -0.045
h3           28 / 1.10 / -0.035   card titles
body-lg      19 / 1.55            lede
body         16 / 1.55            default
body-sm      14 / 1.55            secondary
mono-sm      11 / 1.4 / +0.04     metadata
mono-xs      10 / 1.4 / +0.06     eyebrows, kbd
```

---

## 4 · Color

Eight palettes ship; **`sienna` is canonical** for the desktop client. Other palettes are user-selectable themes.

Each palette defines the same token set. Key roles:

| Token | Role |
|---|---|
| `--cream` | App background |
| `--paper`, `--paper-2` | Card / panel backgrounds, sidebar |
| `--cream-2`, `--cream-3` | Hover/active states, dividers |
| `--ink`, `--ink-2` | Primary text, primary button bg |
| `--ink-soft` | Secondary text |
| `--ink-muted` | Tertiary text, mono metadata |
| `--hairline`, `--hairline-2` | 1px borders |
| `--clay` | Primary accent (eyebrow dots, sync indicator, accents in headlines) |
| `--forest` | Secondary accent (account avatars, status pills) |
| `--good`, `--warn`, `--danger` | Status colors |

See `tokens.json` for all eight palettes' hex values.

**Dark mode** is the `ink` theme. It inverts polarity (cream becomes near-black,
ink becomes warm white) while keeping the same component shapes. Use
`color-mix(in srgb, var(--cream) X%, transparent)` for translucent overlays so
they read correctly in both polarities.

---

## 5 · Spacing, radii, shadows

- **Radii:** 3 / 6 / 10 / 999 (pill). Use small radii — Adagio is not soft.
- **Shadows:** very restrained. `sm` for inputs, `md` for floating panels, `lg` for modals only.
- **Spacing:** prefer multiples of 4 (4, 8, 12, 16, 20, 24, 32, 40, 48, 56, 64, 80, 112).
- **Hairlines:** 1px borders using `var(--hairline)` — many. Adagio leans on hairlines, not shadows, to separate things.

---

## 6 · The slur mark

The core symbol: two notes tied by a curve.

```svg
<svg viewBox="0 0 60 60" fill="none">
  <path d="M8 42 C 18 14, 42 14, 52 42"
        stroke="currentColor" stroke-width="3" stroke-linecap="round" fill="none"/>
  <circle cx="8"  cy="42" r="5" fill="currentColor"/>
  <circle cx="52" cy="42" r="5" class="mark-accent"/>
</svg>
```

The left note is `currentColor` (matches body text), the right note is `var(--clay)`.
Use it as the app icon, in the chrome top-left, and in the footer. The mark scales
all the way down to 12px while staying legible.

---

## 7 · Component vocabulary

Inventory of every component the prototype uses. Numbers in parens are reference IDs you'll see in `source/desktop-app.jsx` and `source/desktop-scenes.jsx`.

### Window chrome (`Chrome`)
- 44px tall, `--paper` background, hairline bottom border
- Left: mark (18px) + wordmark "adagio" (16px, weight 500, tracking −5%)
- Pill toggle: `[ Files | Activity ]` with a 3px inner pad, pill radius
- Center: search bar with cmd-K hint, `--cream-2` fill, pill radius
- Right: bell, settings, avatar (26px, `--forest` circle), then min/max/close

### Sidebar (`Sidebar`, 248px wide)
- `--paper` background, hairline right border
- Account picker at top: 30×30 colored tile (initial), name, host (mono), caret
- Section: "Library" — All files, Favorites, Recent, Shared, Tagged. Active item has `--cream-2` background, 2px `--clay` left edge.
- Section: "Pinned folders" — 4px dot + name; the dot is `--clay` when syncing, `--good` when OK
- Footer: sync status — spinner + "Syncing N files" + size/ETA + 3px progress bar in `--clay`

### File row (`FileRow`)
- 38px-icon + name (1fr) + size (110px, mono) + items (110px, mono) + modified (160px) + status (80px, right-aligned)
- Selected: `--paper-2` background, 2px `--clay` left edge
- Hover: `color-mix(--ink, 4%)` background, share icon appears at right edge
- File glyph: 26×30 paper-2 rectangle with folded corner; kind label centered (PDF/MD/FIG/etc.)
- Folder glyph: 28×22 folder icon outlined in `--forest`

### Status indicators
| State | Icon |
|---|---|
| In sync | 14px `--good` circle with white check |
| Syncing | 14px spinner ring, `--clay` |
| Cloud-only (on-demand) | 16px cloud outline, `--ink-muted` |
| Pinned offline | 14px `--ink` circle with cream check |
| Conflict | 16px triangle/warn, `--danger` |

### Buttons
- **Primary:** `--ink` bg, `--cream` text, 11px / 18px padding, 6px radius
- **Ghost:** transparent, 1px `--ink` border, same padding
- **Flat (toolbar):** transparent → `--paper-2` on hover, 1px `--hairline` border, 7px / 12px padding

### Eyebrow
- Tiny mono uppercase label, prefixed with a 6×6 `--clay` square
- Format: `§ 02 · The client` or `01 · WORDMARK`
- Use to mark sections in long pages

### Pills / chips
- 5px / 11px padding, pill radius, mono 10px caps
- Active: `--ink` bg / `--cream` text
- Inactive: `--paper` bg, hairline border, `--ink-soft` text

### Modal (share dialog uses this pattern)
- Backdrop: `color-mix(var(--ink), 40%)` + 6px blur
- Modal: 540px wide, `--cream` bg, 10px radius, large shadow, 1px hairline
- Header: eyebrow + serif-styled title + close button
- Body: stacked sections separated by hairlines, each with a mono-uppercase label

---

## 8 · Screens

Each screen has a reference screenshot in `screenshots/`.

### 8.1 · `01-onboard.png` — First-run / connect
Five steps. Left rail shows step list with circular markers (clay = active,
forest = done, gray = pending). Right panel:

1. **Welcome.** — Mark + wordmark; three "what you get" checkmarks (No telemetry, Files live in filesystem, AGPL-3.0).
2. **Where does your Nextcloud live?** — URL input with auto-detection. Shows detected Nextcloud version (green dot), auth flow, TLS status, E2EE availability, RTT.
3. **Authorize Adagio in your browser.** — Big dark card with one-time code (mono, letter-spaced), QR code, 5-minute expiry, "Waiting for confirmation at <host>…" with spinner.
4. **Pick a quiet folder.** — Local folder path input, "Browse…" button. Four toggle cards: on-demand (default on), pin pinned folders (default on), smart bandwidth (default on), watch external edits (default off).
5. **Begin softly.** — Forest checkmark + "Connected to <host>" eyebrow + headline. Initial sync progress card (target: bring lightest 200 files first).

Footer: ← Back · "step N of 5" · Continue → (ink button with arrow).

### 8.2 · `02-files.png` — Main file browser
The primary surface. Chrome + sidebar + breadcrumb row + file table + status bar.

- Breadcrumb: `cloud.sound.studio › Sound › **Sound**` (last segment bold)
- Toolbar buttons: `+ New` · `Make available offline` (cloud icon) · `Share` (ink primary)
- Table columns: Name (1fr) · Size · Items · Modified · Status
- Status bar (32px, paper bg): `N items · 1 selected · 2.1 GB local · 84.6 GB on cloud` left; `syncing · ETA 38s · last full sync 11:42` right

The sample data lives in `source/desktop-app.jsx` (`FILE_TREE`).

### 8.3 · `03-activity.png` — Activity feed
- Title row: serif "Activity" + mono caption ("since you last checked in · 12 events") + filter pill segment ([ All · Edits · Shares · Sync · Conflicts ] with counts)
- Buckets: "Today" / "Yesterday" / "Earlier this week" (mono uppercase headers)
- Row: 32px paper-2 circle with action icon (pencil/share/sync/warn/pin/plus) + sentence
- Sentence format: **Who** [verb] *Target* [· with whom] — path · timeframe
- Hover: row gets faint tint and reveals "Open" + (for conflicts) "Resolve" buttons

### 8.4 · `04-share.png` — Share dialog
540px modal overlay. Sections (each separated by a hairline):

1. **With** — chip-input field. Each chip: avatar initials + name + role (mono after a separator) + × dismiss.
2. **Can** + **Until** — two segmented controls side by side. Can: View / Comment / Edit. Until: 24h / 7 days / 30 days / No limit.
3. **Or, a link** — dark ink terminal bar with the share URL (mono) + copy button. Below: three checkboxes (Password protect / Hide download / Notify on open).
4. **A note (optional)** — textarea, paper bg, hairline border.

Footer: lock-icon + "End-to-end encrypted folder" (when applicable) on left; Cancel + "Share with N people" (ink) on right.

### 8.5 · `05-tray.png` — Menu bar / system tray
380px wide popover (no chrome). Pinned to bell icon in OS tray.

- Header: mark + wordmark + status pill (right-aligned, mono, "IN SYNC" with `--good` dot)
- Status block: serif-italic "Up to date" headline + mono meta ("last sync 11:42 · 3,247 files · 86.7 GB")
- Recent: 3 most recent events (24px circle icon + name + relative time)
- Actions list: Open Adagio folder ⌘O · Open in browser ⌘B · Pause syncing ⌘P · Preferences… ⌘,
- Footer: version (mono, left) · Quit Adagio (right)

---

## 9 · Data model implied by the UI

The prototype implies this minimum model — the real implementation will extend it:

```ts
type Account = {
  id: string;             // 'work', 'home'
  name: string;           // 'Work · Sound GmbH'
  host: string;           // 'cloud.sound.studio'
  initial: string;        // 'S' — for the 30×30 tile
  color: string;          // CSS var name or palette role
};

type FileNode = {
  path: string;           // '/Sound/Brand · Adagio'
  name: string;
  kind: 'folder' | 'pdf' | 'md' | 'fig' | 'zip' | 'svg' | 'wav' | 'txt' | string;
  size: number | null;    // bytes; null for folders
  mtime: number;          // unix ms
  status: 'ok' | 'sync' | 'cloud' | 'pin' | 'conflict';
  shareCount?: number;    // for the "people" pill
  itemCount?: number;     // for folders
  etag?: string;
};

type ActivityEvent = {
  id: string;
  who: string;            // 'Mira Olsson', 'You', 'Auto-sync', 'Conflict'
  verb: 'edited' | 'shared' | 'pulled' | 'flagged' | 'pinned' | 'added' | 'declined';
  target: string;         // 'Investor update Q2.md'
  withWhom?: string;      // 'Yui Tanaka'
  where: string;          // '/Sound'
  at: number;             // unix ms
};
```

---

## 10 · Sync state — the most-important visual story

The whole pitch is that **sync state lives in the chrome, never in a popup**. Specifically:

1. **Per-row** — the rightmost cell in the file table is the status indicator.
2. **Sidebar footer** — a persistent spinner + size/ETA + 3px progress bar when there's anything to push or pull.
3. **Status bar (bottom of window)** — running counter ("syncing · ETA 38s · last full sync 11:42").
4. **Tray** — single-line summary of current state.

What should *never* happen:
- A toast notification for "Sync complete"
- A modal for "Conflict detected"
- A badge counter that demands attention
- A progress bar that takes over the window

A conflict shows up as a row status (red triangle) + an entry in the activity feed. The user finds it when they're ready.

---

## 11 · Interaction notes

- **Single-click selects, double-click opens.** Standard. Don't reinvent.
- **Right-click is the same for files and folders** — Share, Make available offline, Pin, Copy link, Reveal in file manager, View on server, Move to trash.
- **Drag-drop** files in or out of the Adagio folder using the OS file manager — never our own DnD reimplementation.
- **Keyboard:** `⌘K` opens search, `⌘N` new folder, `⌘O` open Adagio root, `⌘B` open server in browser, `⌘P` pause sync, `⌘,` preferences, `⌘Q` quit.
- **Focus rings:** 2px `--clay` outline, 2px offset. Visible on keyboard nav, hidden on mouse.
- **Hover delays:** 150ms. Adagio doesn't react instantly to everything — that would be loud.
- **Transitions:** 120–150ms ease-out. No bouncing, no overshoot.
- **Respect prefers-reduced-motion** — all transitions become 0.001ms.

---

## 12 · Notes for implementation (Tauri + Rust)

The prototype is React for fast iteration. Real client should be:

- **Tauri 2** shell with React/TypeScript frontend (port the JSX from `source/`).
- **`cadence`** Rust crate (no Tauri deps) for the sync engine — WebDAV, scheduler, watcher, SQLite state, E2EE. Test it independently.
- **The frontend never touches the network.** All server I/O goes through `cadence` via Tauri `invoke()` commands. The UI is a thin view over a typed IPC.
- **State store:** SQLite for file states, etags, conflicts, activity log. The activity feed is `SELECT … FROM activity ORDER BY at DESC LIMIT 100`.
- **Watcher:** the `notify` crate. Coalesce events with 500ms debounce per path.
- **Auth:** OAuth2 device flow (the onboarding step 3 is exactly this). Token stored in OS keychain via `keyring` crate.
- **Tray:** Tauri 2's `tray` API. The popover is a borderless window pinned to the tray icon.
- **Bundling:** `cargo tauri bundle` → .deb, .rpm, AppImage, .dmg, .msi.

---

## 13 · Quick reference — what lives where in `source/`

| File | What's in it |
|---|---|
| `tokens.css` | The CSS for all 8 palettes; drop into your global stylesheet |
| `logos.jsx` | Three logo directions + the `MarkSlur` SVG component |
| `desktop-app.jsx` | `Chrome`, `Sidebar`, `Icon`, `FileGlyph`, `StatusDot`, `SpinDot`; sample data (`ACCOUNTS`, `FILE_TREE`, `ACTIVITY`) |
| `desktop-scenes.jsx` | `FilesScene`, `OnboardScene`, `ActivityScene`, `ShareDialog`, `Tray`, and the `DesktopApp` dispatcher |
| `website.jsx`, `banners.jsx` | Marketing artifacts — useful for tone/copy but not for the client |

The components use inline styles + CSS custom properties. Port them to whatever
styling solution your real client uses (CSS Modules, vanilla-extract, plain CSS
— anything but Tailwind, which would fight the bespoke type system).

---

## 14 · License

Brand, copy, and screenshots are © Sound GmbH. The source files in this handoff
are made available to you for the purpose of building the Adagio desktop client.
Everything not derived from third-party code (Geist, Instrument Serif fonts) is
yours to use in the client codebase.
