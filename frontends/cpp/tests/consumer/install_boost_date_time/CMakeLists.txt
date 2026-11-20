cmake_minimum_required(VERSION 3.14)
project(hegel_consumer_install_boost_date_time LANGUAGES CXX)

# Consume the Boost.DateTime adapter from an installed hegel. The package
# must have been built and installed with HEGEL_BOOST_DATE_TIME=ON; it then
# finds Boost itself, and hegel::boost_date_time links hegel and the Boost
# headers. See the check-consumer recipe.
find_package(hegel REQUIRED COMPONENTS boost_date_time)

add_executable(consumer ${CMAKE_CURRENT_SOURCE_DIR}/main.cpp)
target_link_libraries(consumer PRIVATE hegel::boost_date_time)
