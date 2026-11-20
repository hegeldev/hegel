cmake_minimum_required(VERSION 3.14)
project(hegel_consumer_install LANGUAGES CXX)

# This package was built without HEGEL_BOOST_DATE_TIME, so it has no
# boost_date_time component and no hegel::boost_date_time target, and asking
# for the component says why.
find_package(hegel QUIET COMPONENTS boost_date_time)
if(hegel_FOUND OR hegel_boost_date_time_FOUND OR TARGET hegel::boost_date_time)
    message(FATAL_ERROR
        "a hegel built without HEGEL_BOOST_DATE_TIME has the boost_date_time component")
endif()
if(NOT hegel_NOT_FOUND_MESSAGE MATCHES "built without -DHEGEL_BOOST_DATE_TIME=ON")
    message(FATAL_ERROR
        "the missing boost_date_time component is not explained: ${hegel_NOT_FOUND_MESSAGE}")
endif()
unset(hegel_NOT_FOUND_MESSAGE)

find_package(hegel REQUIRED)

add_executable(consumer ${CMAKE_CURRENT_SOURCE_DIR}/../main.cpp)
target_link_libraries(consumer PRIVATE hegel::hegel)
