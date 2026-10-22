# libhegel.cmake — the engine this frontend links, exposed as the imported
# target `hegel::libhegel`.
#
# hegel drives Hegel's engine in-process through libhegel's C ABI (the
# `hegel_*` functions declared in hegel.h). The engine is the in-tree
# libhegel: the `hegeltest-c` crate in ../../libhegel, built by cargo into
# the Cargo workspace's `target/` directory (`CARGO_TARGET_DIR` moves it, as
# for cargo). The build runs `cargo build --release -p hegeltest-c` itself
# before linking, so a plain `cmake --build` works from a fresh checkout
# with a Rust toolchain installed.
#
# To link a libhegel built elsewhere (a prebuilt library from a GitHub
# release of the hegeldev/hegel monorepo, say), set
# `-DHEGEL_LIBHEGEL_LIBRARY=/path/to/libhegel_c.<ext>`; cargo is then not
# needed. `-DHEGEL_LIBHEGEL_INCLUDE_DIR=/path/to/dir` points at the
# directory holding that library's `hegel.h` (the default is the in-tree
# header, which matches the in-tree engine).

get_filename_component(HEGEL_MONOREPO_DIR
    "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)

set(HEGEL_LIBHEGEL_INCLUDE_DIR "${HEGEL_MONOREPO_DIR}/libhegel/include"
    CACHE PATH "Directory holding libhegel's hegel.h")
if(NOT EXISTS "${HEGEL_LIBHEGEL_INCLUDE_DIR}/hegel.h")
    message(FATAL_ERROR
        "libhegel: hegel.h not found in ${HEGEL_LIBHEGEL_INCLUDE_DIR}. "
        "Set -DHEGEL_LIBHEGEL_INCLUDE_DIR to the directory holding it.")
endif()

set(HEGEL_LIBHEGEL_LIBRARY "" CACHE FILEPATH
    "A libhegel shared library to link instead of building the in-tree one")

# The shared library cargo produces for the host (its crate name is hegel_c).
if(CMAKE_SYSTEM_NAME STREQUAL "Darwin")
    set(_hegel_local_name "libhegel_c.dylib")
elseif(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    set(_hegel_local_name "libhegel_c.so")
else()
    message(FATAL_ERROR
        "libhegel: unsupported host OS '${CMAKE_SYSTEM_NAME}'. "
        "Set -DHEGEL_LIBHEGEL_LIBRARY to a libhegel built for it.")
endif()

if(HEGEL_LIBHEGEL_LIBRARY)
    set(_hegel_local "${HEGEL_LIBHEGEL_LIBRARY}")
    if(NOT EXISTS "${_hegel_local}")
        message(FATAL_ERROR "HEGEL_LIBHEGEL_LIBRARY does not exist: ${_hegel_local}")
    endif()
    message(STATUS "libhegel: using ${_hegel_local} (HEGEL_LIBHEGEL_LIBRARY)")
else()
    if(DEFINED ENV{CARGO_TARGET_DIR})
        set(_hegel_target_dir "$ENV{CARGO_TARGET_DIR}")
    else()
        set(_hegel_target_dir "${HEGEL_MONOREPO_DIR}/target")
    endif()
    set(_hegel_local "${_hegel_target_dir}/release/${_hegel_local_name}")

    find_program(HEGEL_CARGO cargo)
    if(NOT HEGEL_CARGO)
        message(FATAL_ERROR
            "libhegel: cargo not found; it builds the in-tree engine. Install "
            "a Rust toolchain, or set -DHEGEL_LIBHEGEL_LIBRARY to a libhegel "
            "built elsewhere.")
    endif()
    # cargo is incremental: a no-op once the engine is up to date. Declared
    # as a byproduct so Ninja knows where the library comes from.
    add_custom_target(hegel_libhegel_build
        COMMAND "${HEGEL_CARGO}" build --release -p hegeltest-c
            --manifest-path "${HEGEL_MONOREPO_DIR}/Cargo.toml"
        BYPRODUCTS "${_hegel_local}"
        COMMENT "Building libhegel (cargo build --release -p hegeltest-c)"
        VERBATIM
    )
    message(STATUS "libhegel: building the in-tree engine into ${_hegel_local}")
endif()

add_library(hegel_libhegel SHARED IMPORTED GLOBAL)
set_target_properties(hegel_libhegel PROPERTIES
    IMPORTED_LOCATION "${_hegel_local}"
    INTERFACE_INCLUDE_DIRECTORIES "$<BUILD_INTERFACE:${HEGEL_LIBHEGEL_INCLUDE_DIR}>")
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    # The file is named after its SONAME (libhegel_c.so); treat the path as
    # authoritative rather than recording a load entry for the SONAME.
    set_target_properties(hegel_libhegel PROPERTIES IMPORTED_NO_SONAME TRUE)
endif()
add_library(hegel::libhegel ALIAS hegel_libhegel)

# Absolute path to the resolved library, for install() to ship alongside hegel.
set(HEGEL_LIBHEGEL_RESOLVED "${_hegel_local}" CACHE INTERNAL "Resolved libhegel path")
set(HEGEL_LIBHEGEL_LOCAL_NAME "${_hegel_local_name}" CACHE INTERNAL "libhegel file name")
