# A consumer project built with nix-build against the flake's package, the
# way a downstream Nix user would consume hegel-cpp.
{
  pkgs ? import <nixpkgs> { },
}:
let
  hegel-cpp = import ../../nix;
  fs = pkgs.lib.fileset;
in
pkgs.stdenv.mkDerivation {
  pname = "hegel-cpp-nix-test";
  version = "0.1.0";

  src = fs.toSource {
    root = ./.;
    fileset = fs.unions [
      ./main.cpp
      ./CMakeLists.txt
    ];
  };

  nativeBuildInputs = [
    pkgs.cmake
    pkgs.ninja
  ];

  buildInputs = [ hegel-cpp.packages.${pkgs.stdenv.hostPlatform.system}.default ];
}
