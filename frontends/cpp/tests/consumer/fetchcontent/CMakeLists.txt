cmake_minimum_required(VERSION 3.14)
project(hegel_consumer_fetchcontent LANGUAGES CXX)

if(NOT DEFINED HEGEL_MONOREPO_ROOT)
    message(FATAL_ERROR "HEGEL_MONOREPO_ROOT must be set to the hegel monorepo directory")
endif()
if(NOT DEFINED HEGEL_REF)
    message(FATAL_ERROR "HEGEL_REF must be set to a git ref (SHA/tag/branch)")
endif()

# How a downstream project consumes the frontend: the monorepo repository
# at a `frontends/cpp/<version>` tag, with the frontend as the source
# subdirectory. Here the repository is this checkout and the ref is HEAD.
include(FetchContent)
FetchContent_Declare(
    hegel
    GIT_REPOSITORY file://${HEGEL_MONOREPO_ROOT}
    GIT_TAG ${HEGEL_REF}
    SOURCE_SUBDIR frontends/cpp
)
FetchContent_MakeAvailable(hegel)

add_executable(consumer ${CMAKE_CURRENT_SOURCE_DIR}/../main.cpp)
target_link_libraries(consumer PRIVATE hegel)
