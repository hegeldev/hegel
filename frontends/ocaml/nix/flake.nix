{
  description = "Hegel for OCaml";

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
      # Callers may override `ocamlPackages` to build against a non-default
      # OCaml toolchain (e.g. an oxcaml-flavored scope built via
      # `pkgs.ocaml-ng.mkOcamlPackages`). The default uses `pkgs.ocamlPackages`.
      # `doCheck` toggles `dune runtest` for the listed packages.
      lib.mkHegelOcamlProject =
        {
          pkgs,
          stdenv ? pkgs.stdenv,
          ocamlPackages ? pkgs.ocamlPackages,
          doCheck ? false,
        }:
        let
          duneProjectLines = builtins.filter builtins.isString (
            builtins.split "\n" (builtins.readFile ../dune-project)
          );
          versionLine = builtins.head (
            builtins.filter (l: builtins.match "\\(version .*\\)" l != null) duneProjectLines
          );
          duneProjectVersion = builtins.elemAt (builtins.match "\\(version ([^)]+)\\)" versionLine) 0;
        in
        ocamlPackages.buildDunePackage {
          pname = "hegel";
          version = duneProjectVersion;
          src = ../.;
          duneVersion = "3";
          nativeBuildInputs = [ ocamlPackages.cppo ];
          propagatedBuildInputs = with ocamlPackages; [
            cmdliner
            core
            core_unix
            ctypes
            ctypes-foreign
            dune-site
            ipaddr
            ocplib-endian
            ppx_js_style
            ppx_sexp_conv
            ppxlib
            sexplib0
            yojson
          ];
          checkInputs = with ocamlPackages; [ alcotest ];
          buildPhase = ''
            runHook preBuild
            dune build -p hegel,ppx_hegel_compat,ppx_hegel_test,ppx_hegel_generator -j $NIX_BUILD_CORES
            runHook postBuild
          '';
          checkPhase = ''
            runHook preCheck
            dune runtest -p hegel,ppx_hegel_compat,ppx_hegel_test,ppx_hegel_generator -j $NIX_BUILD_CORES
            runHook postCheck
          '';
          installPhase = ''
            runHook preInstall
            dune install --prefix $out --libdir $OCAMLFIND_DESTDIR \
              hegel ppx_hegel_test ppx_hegel_generator ppx_hegel_compat
            runHook postInstall
          '';
          inherit doCheck;
        };

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            packages = [
              pkgs.ocaml
              pkgs.dune_3
              pkgs.opam
              pkgs.just
            ];
            # The native engine (libhegel) is resolved at runtime by
            # lib/ffi/loader.ml, not provided by Nix: it checks
            # $HEGEL_LIBHEGEL_PATH, then the installed package's libhegel
            # site, then the monorepo's in-tree cargo build
            # (target/{release,debug}/ of the enclosing checkout). Build it
            # with `cargo build --release -p hegeltest-c` or set
            # HEGEL_LIBHEGEL_PATH.
          };
        }
      );
    };
}
