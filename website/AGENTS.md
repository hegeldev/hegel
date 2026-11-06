# Website

The source of https://hegel.dev: an [Astro](https://astro.build/) site using
[Starlight](https://starlight.astro.build/) with the Rapide theme, hosted on
Vercel and deployed there by CI from `main`.

## Commands

```bash
just website::setup   # npm ci
just website::serve   # dev server (alias: dev)
just website::build   # production build into dist/
```

## Architecture

- **Deployment**: Vercel, configured in `vercel.json` and in `astro.config.mjs` through the `@astrojs/vercel` adapter (with web analytics). The Vercel project (team `hegeldev`, project `website`, root directory `website/`) is not connected to the GitHub repository: the `deploy` job of `.github/workflows/website.yml` builds the site with the Vercel CLI (`vercel pull`, `vercel build`, `vercel deploy --prebuilt`, run from the repo root) using the `VERCEL_TOKEN` repository secret. A push to `main` that changes `website/` deploys to production; a pull request from this repository gets a preview deployment, whose URL is in the job summary; a pull request from a fork gets no deployment, since GitHub gives it no secrets. Never deploy by hand.
- **Content**: Markdown/MDX in `src/content/docs/`; Starlight generates a page from each.
- **Sidebar**: `astro.config.mjs`, under `starlight({ sidebar: [...] })`.
- **API references**: `public/cpp/`, `public/typescript/` and `public/ocaml/` are generated, served at hegel.dev/cpp, /typescript and /ocaml. Never edit them by hand: the `release-<lang>-docs` jobs of `.github/workflows/ci.yml` rebuild them on every release of that frontend and commit them to `main` (`.github/scripts/push_<lang>_docs.py`).
- **Go module redirect**: `public/go/hegel.html` serves the `go-import` meta tag so `go get hegel.dev/go/hegel` resolves via [hegel-go-modproxy](https://github.com/hegeldev/hegel-go-modproxy), a GOPROXY that serves the module with its git-lfs files. `cleanUrls` in `vercel.json` serves it at `/go/hegel`.
- **Custom CSS**: `src/styles/style.css` overrides Starlight defaults (text color, line height, admonition styling). External sidebar links use the `.external-link` class, which appends the icon in `src/assets/external-link.svg`.
- **Images**: the Hedgel mascot images live in `src/assets/` and `public/`.
- **Git LFS**: `.gitattributes` stores every image and everything under `public/cpp/` and `public/typescript/` in Git LFS, so a checkout without `git lfs` has pointer files there and the build fails. CI checks the site out with `lfs: true`.

## CI

`.github/workflows/website.yml` builds the site (`build`, on every change, without secrets) and deploys it (`deploy`, see above). A change confined to `website/` runs only that workflow: the lints, component tests and release are skipped (see the `changes` job of `ci.yml`). The site is not a released component and needs no release file.
