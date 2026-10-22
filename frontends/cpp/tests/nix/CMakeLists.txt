cmake_minimum_required(VERSION 3.14)
project(hegel-cpp-nix-test VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# hegel::hegel pulls in the libhegel shared engine (which resolves the
# hegel_* symbols in the static archive) and its runtime RPATH via
# hegelConfig.cmake.
find_package(hegel REQUIRED)

add_executable(nix_test main.cpp)
target_link_libraries(nix_test PRIVATE hegel::hegel)

include(GNUInstallDirs)
install(TARGETS nix_test RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
