# flake-compat entry point. The flake reads files outside this directory
# (the frontend's sources and the root flake), so it is evaluated against a
# fetchGit copy of the whole repository: only tracked files, never build
# output, reach the store.
let
  repo = builtins.fetchGit ../../..;
in
(import
  (
    let
      lock = builtins.fromJSON (builtins.readFile ./flake.lock);
    in
    fetchTarball {
      url =
        lock.nodes.flake-compat.locked.url
          or "https://github.com/edolstra/flake-compat/archive/${lock.nodes.flake-compat.locked.rev}.tar.gz";
      sha256 = lock.nodes.flake-compat.locked.narHash;
    }
  )
  {
    src = repo // {
      outPath = repo.outPath + "/frontends/cpp/nix";
    };
  }
).defaultNix
