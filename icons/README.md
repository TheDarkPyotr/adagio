# Adagio · Application Icon

The chosen direction: **Slur** — two notes tied by a curve. The brand mark inside a rounded square tile.

## What's in this folder

```
icon-export/
├── README.md                           — this file
├── svg/
│   ├── adagio-icon.svg                 — master vector (light tile, recommended)
│   ├── adagio-icon-dark.svg            — dark variant
│   ├── adagio-icon-mono.svg            — single-color, transparent (tray / template)
│   └── adagio-icon-transparent.svg     — no tile background
├── png/                                — light tile PNGs (default)
│   ├── adagio-icon-16.png
│   ├── adagio-icon-24.png
│   ├── adagio-icon-32.png
│   ├── adagio-icon-48.png
│   ├── adagio-icon-64.png
│   ├── adagio-icon-128.png
│   ├── adagio-icon-256.png
│   ├── adagio-icon-512.png
│   └── adagio-icon-1024.png
├── png-dark/                           — same sizes, dark tile
└── png-mono/                           — same sizes, single black on transparent
```

The SVGs are the source of truth. PNGs were rasterized from them at the standard
desktop-icon sizes.

## Colors

| Token | Hex |
|---|---|
| Tile (light) | `#f5f1ea` *(cream)* |
| Tile (dark)  | `#15171a` *(ink)* |
| Left note + arc | `#15171a` *(ink)* — `#f5f1ea` in dark variant |
| Right note (accent) | `#c8542a` *(clay)* |

Geometry: rounded square, corner radius **21% of side** (Apple-standard).

---

## Packaging the icon by platform

### macOS · `.icns`

The `iconutil` tool (built into macOS) needs a folder named `Adagio.iconset/`
with files named to its convention. Run from this folder:

```bash
mkdir Adagio.iconset
cp png/adagio-icon-16.png    Adagio.iconset/icon_16x16.png
cp png/adagio-icon-32.png    Adagio.iconset/icon_16x16@2x.png
cp png/adagio-icon-32.png    Adagio.iconset/icon_32x32.png
cp png/adagio-icon-64.png    Adagio.iconset/icon_32x32@2x.png
cp png/adagio-icon-128.png   Adagio.iconset/icon_128x128.png
cp png/adagio-icon-256.png   Adagio.iconset/icon_128x128@2x.png
cp png/adagio-icon-256.png   Adagio.iconset/icon_256x256.png
cp png/adagio-icon-512.png   Adagio.iconset/icon_256x256@2x.png
cp png/adagio-icon-512.png   Adagio.iconset/icon_512x512.png
cp png/adagio-icon-1024.png  Adagio.iconset/icon_512x512@2x.png

iconutil -c icns Adagio.iconset
# → Adagio.icns
```

Tauri reads this as `src-tauri/icons/icon.icns`.

### Windows · `.ico`

Use ImageMagick:

```bash
magick png/adagio-icon-16.png png/adagio-icon-24.png \
       png/adagio-icon-32.png png/adagio-icon-48.png \
       png/adagio-icon-64.png png/adagio-icon-128.png \
       png/adagio-icon-256.png \
       icon.ico
```

Tauri reads this as `src-tauri/icons/icon.ico`.

### Linux · PNG hierarchy + SVG

Most freedesktop themes look for these paths under
`~/.local/share/icons/hicolor/` (or `/usr/share/icons/hicolor/`):

```
hicolor/
├── 16x16/apps/adagio.png      ← png/adagio-icon-16.png
├── 24x24/apps/adagio.png      ← png/adagio-icon-24.png
├── 32x32/apps/adagio.png      ← png/adagio-icon-32.png
├── 48x48/apps/adagio.png      ← png/adagio-icon-48.png
├── 64x64/apps/adagio.png      ← png/adagio-icon-64.png
├── 128x128/apps/adagio.png    ← png/adagio-icon-128.png
├── 256x256/apps/adagio.png    ← png/adagio-icon-256.png
├── 512x512/apps/adagio.png    ← png/adagio-icon-512.png
└── scalable/apps/adagio.svg   ← svg/adagio-icon.svg
```

Run `gtk-update-icon-cache` after installing.

### Tauri 2 · Drop-in

Tauri 2's `tauri.conf.json` expects an array under `bundle.icon`. The simplest
setup is to drop all PNGs + the .icns + .ico into `src-tauri/icons/` and reference:

```json
{
  "bundle": {
    "icon": [
      "icons/icon-32.png",
      "icons/icon-128.png",
      "icons/icon-128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

Or just run `cargo tauri icon path/to/icon-1024.png` and Tauri's CLI generates
the full set in `src-tauri/icons/` for you.

---

## Tray / menu-bar icons

For tray icons, use the **mono** PNGs. Most platforms expect a single-color,
transparent-background image that they tint themselves to match the system bar.

- **macOS:** name it `iconTemplate.png` (and `iconTemplate@2x.png`) — macOS auto-inverts.
- **Linux (GNOME / KDE):** the mono PNGs work directly; theming engines respect alpha.
- **Windows:** use the regular light PNGs; Windows doesn't auto-tint.

Recommended tray sizes: **16**, **24**, **32**, **48** (the 48 covers Wayland HiDPI).

---

## Favicons

For the web (companion site + GitHub README badges):

```html
<link rel="icon" type="image/svg+xml" href="adagio-icon.svg">
<link rel="alternate icon" type="image/png" sizes="32x32" href="adagio-icon-32.png">
<link rel="apple-touch-icon" href="adagio-icon-256.png">
```

---

## Notes

- The PNG renderer in your browser may anti-alias slightly differently than
  `librsvg` or Inkscape. If you need pixel-perfect 16/24px outputs for Windows
  `.ico`, regenerate those sizes specifically through:

  ```bash
  rsvg-convert -w 16 -h 16 svg/adagio-icon.svg > png/adagio-icon-16.png
  ```

- For Tauri's auto-generated icon set, start from `png/adagio-icon-1024.png`
  — it's the highest-fidelity source.

- The dark variant exists for cases where the surrounding chrome is light
  (rare on desktop; mostly useful for marketing materials).

## License

Brand and design © Sound GmbH. Use within the Adagio desktop client codebase.
