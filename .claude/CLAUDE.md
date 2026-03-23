# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Hegel's documentation website, built with [Astro](https://astro.build/) and [Starlight](https://starlight.astro.build/) (Starlight theme: Rapide). Deployed to Vercel.

## Commands

```bash
npm install          # Install dependencies
npm run dev          # Start dev server
npm run build        # Production build
npm run preview      # Preview production build locally
```

## Architecture

- **Framework**: Astro 6 with Starlight docs theme
- **Deployment**: Vercel (configured in `vercel.json` and `astro.config.mjs` via `@astrojs/vercel` adapter with web analytics)
- **Content**: Markdown/MDX files in `src/content/docs/` — Starlight auto-generates pages from these
- **Sidebar**: Configured in `astro.config.mjs` under `starlight({ sidebar: [...] })`
- **Go module redirect**: `public/go/hegel.html` serves the `go-import` meta tag so `go get hegel.dev/go/hegel` resolves to the GitHub repo. The Vercel redirect in `vercel.json` routes `/go/hegel/` to this file.
- **Custom CSS**: `src/styles/style.css` overrides Starlight defaults (text color, line height, admonition styling). External sidebar links use the `.external-link` class which appends an icon via `src/assets/external-link.svg`.
- **Images**: Hedgel mascot images in `src/assets/`, tracked via Git LFS
