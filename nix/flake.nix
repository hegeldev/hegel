{
  description = "Hegel: the libhegel engine and its language frontends";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-compat.url = "https://flakehub.com/f/edolstra/flake-compat/1.tar.gz";
  };

  outputs =
    {
      self,
      nixpkgs,
      ...
    }:
    let
      forAllSystems = nixpkgs.lib.genAttrs nixpkgs.lib.systems.flakeExposed;
    in
    {
      # Build the native engine cdylib (`lib/libhegel_c.so` or `.dylib`, plus
      # `lib/libhegel.so` on Linux for older callers) and its C header
      # (`include/hegel.h`) for the C-ABI bindings and the language frontends'
      # flakes (frontends/cpp/nix). Callers may override `cargoDeps` to plug in
      # their own vendoring; the default uses `importCargoLock` against the
      # workspace `Cargo.lock`.
      lib.mkLibhegel =
        {
          pkgs,
          cargoDeps ? pkgs.rustPlatform.importCargoLock { lockFile = ../Cargo.lock; },
        }:
        let
          ext = pkgs.stdenv.hostPlatform.extensions.sharedLibrary;
          cargoTomlLines = builtins.filter builtins.isString (
            builtins.split "\n" (builtins.readFile ../libhegel/Cargo.toml)
          );
          versionLine = builtins.head (
            builtins.filter (l: builtins.match ''version = "[^"]+"'' l != null) cargoTomlLines
          );
          version = builtins.elemAt (builtins.match ''version = "([^"]+)"'' versionLine) 0;
        in
        pkgs.rustPlatform.buildRustPackage {
          pname = "libhegel";
          inherit version cargoDeps;
          src = ../.;
          cargoBuildFlags = [
            "-p"
            "hegeltest-c"
          ];
          doCheck = false;
          # buildRustPackage's default install only handles binaries; we need
          # the cdylib. The artifact may land in target/<triple>/release/ or
          # target/release/ depending on whether --target is in play.
          postInstall = ''
            mkdir -p $out/lib $out/include
            cp "$(find target -name 'libhegel_c${ext}' -path '*/release/*' | head -n1)" \
              "$out/lib/libhegel_c${ext}"
            cp libhegel/include/hegel.h $out/include/hegel.h
          ''
          + pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            ln -s libhegel_c.so $out/lib/libhegel.so
          '';
        };

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            packages = [
              pkgs.cargo
              pkgs.rustc
              pkgs.rustfmt
              pkgs.clippy
              pkgs.rust-analyzer
              pkgs.just
              pkgs.cargo-expand
              pkgs.python3
            ];
          };
        }
      );
    };
}
