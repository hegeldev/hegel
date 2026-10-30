RELEASE_TYPE: patch

This patch adds `GIT_CONFIG lfs.fetchexclude=website` to the recommended `FetchContent_Declare` in the README and the API reference. The hegeldev/hegel repository also holds the hegel.dev website, whose images and generated API references are stored in Git LFS, and with this line a machine that has git-lfs installed no longer downloads them when fetching Hegel.
