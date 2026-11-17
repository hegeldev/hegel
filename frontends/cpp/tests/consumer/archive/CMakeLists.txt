cmake_minimum_required(VERSION 3.14)
project(hegel_consumer_archive LANGUAGES CXX)

if(NOT DEFINED HEGEL_ARCHIVE)
    message(FATAL_ERROR "HEGEL_ARCHIVE must be set to a hegel-cpp-<version>.tar.gz source archive")
endif()
if(NOT DEFINED HEGEL_ARCHIVE_SHA256)
    message(FATAL_ERROR "HEGEL_ARCHIVE_SHA256 must be set to the archive's SHA-256")
endif()

# How a downstream project consumes a release without git: the source
# archive attached to the `frontends/cpp/<version>` GitHub release, which
# keeps the monorepo's layout, so the frontend is the same source
# subdirectory. Here the archive is a local file built from this checkout
# (scripts/check-source-archive.py), and it builds its own libhegel.
include(FetchContent)
FetchContent_Declare(
    hegel
    URL ${HEGEL_ARCHIVE}
    URL_HASH SHA256=${HEGEL_ARCHIVE_SHA256}
    SOURCE_SUBDIR frontends/cpp
)
FetchContent_MakeAvailable(hegel)

add_executable(consumer ${CMAKE_CURRENT_SOURCE_DIR}/../main.cpp)
target_link_libraries(consumer PRIVATE hegel)
