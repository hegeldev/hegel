# Website

The source of https://hegel.dev: an [Astro](https://astro.build/) site using
[Starlight](https://starlight.astro.build/) with the Rapide theme, deployed by
Vercel from `website/` on `main`.

## Commands

```bash
just website::setup   # npm ci
just website::serve   # dev server (alias: dev)
just website::build   # production build into dist/
```

## Architecture

- **Deployment**: Vercel, configured in `vercel.json` and in `astro.config.mjs` through the `@astrojs/vercel` adapter (with web analytics). The Vercel project's root directory is `website/`.
- **Content**: Markdown/MDX in `src/content/docs/`; Starlight generates a page from each.
- **Sidebar**: `astro.config.mjs`, under `starlight({ sidebar: [...] })`.
- **API references**: `public/cpp/`, `public/typescript/` and `public/ocaml/` are generated, served at hegel.dev/cpp, /typescript and /ocaml. Never edit them by hand: the `release-<lang>-docs` jobs of `.github/workflows/ci.yml` rebuild them on every release of that frontend and commit them to `main` (`.github/scripts/push_<lang>_docs.py`).
- **Go module redirect**: `public/go/hegel.html` serves the `go-import` meta tag so `go get hegel.dev/go/hegel` resolves via [hegel-go-modproxy](https://github.com/hegeldev/hegel-go-modproxy), a GOPROXY that serves the module with its git-lfs files. `cleanUrls` in `vercel.json` serves it at `/go/hegel`.
- **Custom CSS**: `src/styles/style.css` overrides Starlight defaults (text color, line height, admonition styling). External sidebar links use the `.external-link` class, which appends the icon in `src/assets/external-link.svg`.
- **Images**: the Hedgel mascot images live in `src/assets/` and `public/`.

## CI

`.github/workflows/website.yml` builds the site. A change confined to `website/` runs only that job: the lints, component tests and release are skipped (see the `changes` job of `ci.yml`). The site is not a released component and needs no `RELEASE.md`.
