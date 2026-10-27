{
  description = "Hegel for C++";

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

      # The engine is the in-tree libhegel, built from source by the
      # monorepo's flake (nix/flake.nix at the repository root), never a
      # prebuilt release asset.
      hegel = import ../../../nix/flake.nix;
      mkLibhegel =
        pkgs:
        (hegel.outputs {
          self = hegel;
          inherit nixpkgs;
        }).lib.mkLibhegel
          { inherit pkgs; };

      version =
        let
          lines = builtins.filter builtins.isString (
            builtins.split "\n" (builtins.readFile ../CMakeLists.txt)
          );
          projectLine = builtins.head (
            builtins.filter (l: builtins.match "project\\(hegel-cpp VERSION .*" l != null) lines
          );
        in
        builtins.elemAt (builtins.match "project\\(hegel-cpp VERSION ([^ ]+) .*" projectLine) 0;

      # The FetchContent sources CMakeLists.txt and tests/CMakeLists.txt
      # declare, fetched here because the build sandbox has no network.
      fetchDeps = pkgs: {
        reflectcpp = pkgs.fetchFromGitHub {
          owner = "getml";
          repo = "reflect-cpp";
          tag = "v0.22.0";
          hash = "sha256-5Og3+dM3QuCX6sT+6Rz8vwvyzQb+8qz10ROk9yOMPgE=";
        };
        approvaltests = pkgs.fetchFromGitHub {
          owner = "approvals";
          repo = "ApprovalTests.cpp";
          tag = "v.10.13.0";
          hash = "sha256-Z9VI+OmvGyzBZ5hU0O+xn2hgNMDYTUQCl+k/i965n5Q=";
        };
      };

      mkFetchContentFlags =
        pkgs:
        let
          lib = pkgs.lib;
          deps = fetchDeps pkgs;
        in
        lib.mapAttrsToList (k: v: lib.cmakeFeature k (toString v)) {
          FETCHCONTENT_SOURCE_DIR_REFLECTCPP = deps.reflectcpp;
          FETCHCONTENT_SOURCE_DIR_GOOGLETEST = pkgs.gtest.src;
          FETCHCONTENT_SOURCE_DIR_APPROVALTESTS = deps.approvaltests;
          FETCHCONTENT_FULLY_DISCONNECTED = "ON";
        };

      # The builder consumers call: the hegel library, headers and CMake
      # package, with the engine shipped beside the static archive.
      mkHegelCppProject =
        {
          pkgs,
          stdenv ? pkgs.stdenv,
        }:
        let
          lib = pkgs.lib;
          libhegel = mkLibhegel pkgs;
          ext = pkgs.stdenv.hostPlatform.extensions.sharedLibrary;
          fs = lib.fileset;
        in
        stdenv.mkDerivation {
          pname = "hegel-cpp";
          inherit version;

          src = fs.toSource {
            root = ./..;
            fileset = fs.unions [
              ./../cmake
              ./../CMakeLists.txt
              ./../src
              ./../include
              (fs.difference ./../tests ./../tests/nix)
              ./../docs
            ];
          };

          nativeBuildInputs = [
            pkgs.cmake
            pkgs.ninja
          ];

          buildInputs = [ libhegel ];

          cmakeFlags = (mkFetchContentFlags pkgs) ++ [
            (lib.cmakeFeature "HEGEL_LIBHEGEL_LIBRARY" "${libhegel}/lib/libhegel_c${ext}")
            (lib.cmakeFeature "HEGEL_LIBHEGEL_INCLUDE_DIR" "${libhegel}/include")
          ];

          doCheck = true;
          checkPhase = ''
            runHook preCheck
            ctest --test-dir tests --output-on-failure --no-tests=error
            runHook postCheck
          '';
        };
    in
    {
      lib = {
        inherit mkHegelCppProject;
      };

      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = mkHegelCppProject { inherit pkgs; };
          libhegel = mkLibhegel pkgs;
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            inputsFrom = [ self.packages.${system}.default ];
            packages = [
              pkgs.clang-tools
              pkgs.cargo
              pkgs.rustc
              pkgs.just
            ];
          };
        }
      );
    };
}
