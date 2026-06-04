# Adagio — website

The marketing site for **Adagio**, a minimal desktop client for Nextcloud.

Static, no build step. Plain HTML + CSS + one image. Ready to host on **GitHub Pages**.

```
website-static/
├── index.html              the page
├── styles.css              flat night-blue theme + layout
├── .nojekyll               tells GitHub Pages to serve files as-is
├── README.md
└── assets/
    ├── favicon.svg         tab icon
    ├── mark.svg            the slur mark
    └── file-browser.png    product screenshot (the v0.1.0 file browser)
```

## Preview locally

No tooling required — just open `index.html`, or serve the folder:

```bash
python3 -m http.server 8000   # then open http://localhost:8000
```

## Deploy to GitHub Pages

**Option A — project site from a folder**

1. Commit this folder to your repo (e.g. at the repo root, or under `docs/`).
2. Repo → **Settings → Pages**.
3. **Source:** *Deploy from a branch*. Pick your branch and the folder
   (`/root` if these files are at the top level, or `/docs` if you put them there).
4. Save. Your site goes live at `https://<user>.github.io/<repo>/`.

**Option B — dedicated `gh-pages` branch**

```bash
git checkout --orphan gh-pages
git rm -rf .
cp -r website-static/* website-static/.nojekyll .
git add . && git commit -m "Deploy site"
git push origin gh-pages
```

Then set **Settings → Pages → Source** to the `gh-pages` branch, `/root`.

**Custom domain (adagio.so)**

Add a file named `CNAME` next to `index.html` containing your domain
(`adagio.so`), then point a DNS `CNAME`/`A` record at GitHub Pages.

## Notes

- `.nojekyll` is included so GitHub Pages serves the files verbatim (no Jekyll
  processing) — keeps things fast and predictable.
- All links (Download, GitHub, Docs, View source) are placeholders (`#`) — wire
  them to your real release artifacts and repo.
- The product screenshot is `assets/file-browser.png`. Replace it with a fresh
  capture when the UI changes; the `<img>` is 900×700.
- Fonts load from Google Fonts (Geist + Geist Mono). To self-host for full
  offline/no-tracking, download the WOFF2 files and swap the `<link>` for a
  local `@font-face` block.

## License

Site source MIT. Brand, copy, and screenshot © Sound GmbH.
