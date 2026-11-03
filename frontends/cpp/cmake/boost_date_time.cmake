# The Boost.DateTime adapter (include/hegel/extras/boost_date_time.h), as the
# INTERFACE target hegel::boost_date_time. Included only when
# HEGEL_BOOST_DATE_TIME is ON, so the core library never needs Boost.
#
# An installed Boost (1.73 or later, where Boost.DateTime is header-only) is
# used when CMake finds one. Otherwise Boost's release tarball (the b2-nodocs
# asset of its GitHub release, pinned to the SHA-256 Boost publishes beside
# it) is fetched and its headers used; nothing of it is built.

set(HEGEL_BOOST_VERSION 1.86.0)

find_package(Boost 1.73 CONFIG QUIET)

if(TARGET Boost::headers)
    set(_hegel_boost_headers Boost::headers)
else()
    if(CMAKE_VERSION VERSION_LESS 3.24)
        message(FATAL_ERROR
            "HEGEL_BOOST_DATE_TIME without an installed Boost needs CMake 3.24 "
            "or later to fetch Boost")
    endif()
    include(FetchContent)
    FetchContent_Declare(
        hegel_boost
        URL https://github.com/boostorg/boost/releases/download/boost-${HEGEL_BOOST_VERSION}/boost-${HEGEL_BOOST_VERSION}-b2-nodocs.tar.xz
        URL_HASH SHA256=a4d99d032ab74c9c5e76eddcecc4489134282245fffa7e079c5804b92b45f51d
        DOWNLOAD_EXTRACT_TIMESTAMP TRUE
        # A directory without a CMakeLists.txt: fetch only.
        SOURCE_SUBDIR hegel-headers-only
    )
    FetchContent_MakeAvailable(hegel_boost)
    add_library(hegel_boost_headers INTERFACE)
    target_include_directories(hegel_boost_headers SYSTEM INTERFACE "${hegel_boost_SOURCE_DIR}")
    set(_hegel_boost_headers hegel_boost_headers)
endif()

add_library(hegel_boost_date_time INTERFACE)
target_link_libraries(hegel_boost_date_time INTERFACE hegel ${_hegel_boost_headers})
add_library(hegel::boost_date_time ALIAS hegel_boost_date_time)
