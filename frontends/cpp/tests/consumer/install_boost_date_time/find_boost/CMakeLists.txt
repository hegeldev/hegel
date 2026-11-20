cmake_minimum_required(VERSION 3.14)
project(hegel_consumer_find_boost LANGUAGES CXX)

# Configures only where CMake finds the Boost that an installed
# hegel::boost_date_time needs. The check-consumer recipe runs this first
# and skips the install_boost_date_time mode when it fails.
find_package(Boost 1.73 CONFIG REQUIRED)
