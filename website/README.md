# Website

Source code for https://hegel.dev.

## Developing

The images and the generated C++ and TypeScript references are stored in [Git LFS](https://git-lfs.com/): install it and run `git lfs install` before cloning, or `git lfs pull` after.

* Install dependencies: `just website::setup` (or `npm ci` here)
* Run dev server: `just website::serve` (or `npm run dev` here)
