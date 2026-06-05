# Data Model: Fumadocs Documentation Site

**Feature**: 018-mkdocs-docs-site

This feature produces a static documentation site. There is no runtime database. The "data model" describes the configuration schema and file-system conventions that govern the site.

---

## Navigation Entry (meta.json)

Represents one section's sidebar order defined in `content/docs/<section>/meta.json`.

| Field | Type | Validation |
|-------|------|------------|
| `title` | string | Non-empty; ≤ 60 chars (sidebar truncation) |
| `pages` | string[] | Ordered list of page slugs (filename without `.mdx`) |
| `defaultOpen` | boolean | Optional; whether section is expanded by default |

**Constraint**: Every slug in `pages` must resolve to an existing `.mdx` file in the same directory; violation causes `next build` to fail.

---

## Documentation Page (MDX file)

An MDX file under `docs-site/content/docs/` that becomes a static HTML page.

| Field | Type | Validation |
|-------|------|------------|
| `file_path` | string | Relative to `docs-site/content/docs/`; kebab-case; `.mdx` extension |
| `title` | string | Required frontmatter field; used in sidebar and `<title>` |
| `description` | string | Optional frontmatter; used in `<meta name="description">` and search |
| `section` | string | One of: `getting-started`, `user-guide`, `cli-reference`, `daemon-ipc`, `architecture`, `contributing` |
| `edit_url` | string | Auto-generated in `docs/[[...slug]]/page.tsx` as `https://github.com/…/blob/main/docs-site/content/docs/<path>.mdx` |

**Frontmatter schema**:
```yaml
---
title: string          # required
description: string    # optional
---
```

**State transitions**: Pages do not have lifecycle state — they exist as files or do not.

---

## Theme Override

The Tailwind CSS variable overrides that map Adagio midnight tokens onto `fumadocs-ui` design variables.

| Token set | Source file | Target |
|-----------|-------------|--------|
| Background/surface colours | `docs-site/app/globals.css` | CSS custom properties on `:root[data-theme="dark"]` |
| Typography colours | `docs-site/app/globals.css` | `--foreground`, `--muted-foreground` etc. |
| Accent / primary colours | `docs-site/app/globals.css` | `--primary`, `--ring` |
| Font face declarations | `docs-site/app/layout.tsx` | `next/font/local` → CSS variable `--font-sans`, `--font-mono` |
| Code block colours | `docs-site/app/globals.css` | Shiki `vitesse-dark`-based override or custom Shiki theme |

---

## Site Configuration Schema

### `docs-site/source.config.ts`

```ts
defineCollections({
  type: 'doc',
  dir: 'content/docs',
})
```

### `docs-site/next.config.mjs` (key fields)

```ts
{
  output: 'export',           // static HTML export for GitHub Pages
  basePath: '/adagio',        // if hosted at /adagio path on GitHub Pages
  images: { unoptimized: true }, // required for static export
}
```

### `docs-site/tailwind.config.ts` (key fields)

```ts
{
  content: ['./app/**/*.{ts,tsx}', './content/**/*.mdx', './node_modules/fumadocs-ui/dist/**/*.js'],
  presets: [require('fumadocs-ui/tailwind-plugin')],
}
```

---

## GitHub Actions Workflow

The CI/CD pipeline entity.

| Field | Type | Value |
|-------|------|-------|
| `trigger` | push | branch `main`, paths `docs-site/**` or `.github/workflows/docs.yml` |
| `node_version` | string | `"22"` (LTS) |
| `cache_key` | string | Hash of `docs-site/package-lock.json` |
| `install_command` | string | `npm ci` (inside `docs-site/`) |
| `build_command` | string | `npm run build` → `next build` |
| `output_dir` | string | `docs-site/out/` |
| `deploy_action` | string | `peaceiris/actions-gh-pages` → `gh-pages` branch |
