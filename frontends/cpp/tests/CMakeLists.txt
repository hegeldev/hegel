enable_testing()

FetchContent_Declare(
    googletest
    GIT_REPOSITORY https://github.com/google/googletest.git
    GIT_TAG v1.14.0
)
set(gtest_force_shared_crt ON CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(googletest)
include(GoogleTest)

# Snapshot ("approval") testing for full-output assertions. ApprovalTests
# is fetched at configure time, so sandboxed builds without network access
# (e.g. nix) set HEGEL_APPROVAL_TESTS=OFF; the test suites that contain
# approval tests are skipped there.
option(HEGEL_APPROVAL_TESTS
    "Build the test suites that use ApprovalTests snapshots" ON)
if(HEGEL_APPROVAL_TESTS)
    FetchContent_Declare(
        ApprovalTests
        GIT_REPOSITORY https://github.com/approvals/ApprovalTests.cpp.git
        GIT_TAG v.10.13.0
    )
    FetchContent_MakeAvailable(ApprovalTests)
endif()

# Interface target shared by all tests that use common/utils.h.
# Provides include path and links gtest for the FAIL() macro.
add_library(hegel_tests_common INTERFACE)
target_include_directories(hegel_tests_common
    INTERFACE
        ${CMAKE_CURRENT_SOURCE_DIR}
)
target_link_libraries(hegel_tests_common
    INTERFACE
        hegel
        GTest::gtest
)

# -----------------------------------------------------------------------------
# hegel_add_test(NAME <name> SOURCE <path> [USE_UTILS] [INTERNAL] [APPROVALS])
#
#   Registers a GTest-based test executable and gtest_discover_tests target.
#
#   NAME         - target name
#   SOURCE       - relative path to the .cpp file
#   USE_UTILS    - if set, link against hegel_tests_common (adds tests/ to
#                  the include path). If not set, link against hegel directly.
#   INTERNAL     - if set, also add src/ to the include path (for unit
#                  tests against private headers).
#   APPROVALS    - if set, the test contains approval (snapshot) tests: the
#                  executable uses common/approval_main.cpp as its main and
#                  links ApprovalTests instead of gtest_main.
# -----------------------------------------------------------------------------
function(hegel_add_test)
    cmake_parse_arguments(HAT
        "USE_UTILS;INTERNAL;APPROVALS"
        "NAME;SOURCE"
        ""
        ${ARGN})

    if(HAT_APPROVALS AND NOT HEGEL_APPROVAL_TESTS)
        return()
    endif()

    set(HAT_SOURCES ${HAT_SOURCE})
    if(HAT_APPROVALS)
        list(APPEND HAT_SOURCES common/approval_main.cpp)
        set(HAT_MAIN_LIBS ApprovalTests::ApprovalTests)
    else()
        set(HAT_MAIN_LIBS GTest::gtest_main)
    endif()

    add_executable(${HAT_NAME} ${HAT_SOURCES})
    if(HAT_USE_UTILS)
        target_link_libraries(${HAT_NAME}
            PRIVATE hegel_tests_common ${HAT_MAIN_LIBS})
    else()
        target_link_libraries(${HAT_NAME}
            PRIVATE hegel GTest::gtest ${HAT_MAIN_LIBS})
    endif()

    if(HAT_INTERNAL)
        target_include_directories(${HAT_NAME}
            PRIVATE
                ${PROJECT_SOURCE_DIR}/src
        )
    endif()

    gtest_discover_tests(${HAT_NAME})
endfunction()

foreach(name IN ITEMS integers floats strings collections flatmap
        mixed_types)
    hegel_add_test(NAME test_shrink_quality_${name}
        SOURCE shrink_quality/test_${name}.cpp
        USE_UTILS)
endforeach()

foreach(name IN ITEMS integers floats strings collections one_of)
    hegel_add_test(NAME test_find_quality_${name}
        SOURCE find_quality/test_${name}.cpp
        USE_UTILS)
endforeach()

# Example / integration property tests.
hegel_add_test(NAME property_tests SOURCE property_tests.cpp)

# Run configuration: settings, profiles, phases, backend, database, health
# checks, run errors, and reproduce-failure blobs.
hegel_add_test(NAME test_settings SOURCE test_settings.cpp)
# Run error kinds mapped to their exception types.
hegel_add_test(NAME test_run_errors SOURCE test_run_errors.cpp INTERNAL)

# Generators, split by family.
hegel_add_test(NAME test_generators_numeric
    SOURCE test_generators_numeric.cpp
    USE_UTILS)
hegel_add_test(NAME test_generators_strings
    SOURCE test_generators_strings.cpp)
hegel_add_test(NAME test_generators_grammar
    SOURCE test_generators_grammar.cpp)
hegel_add_test(NAME test_generators_collections
    SOURCE test_generators_collections.cpp
    USE_UTILS)
hegel_add_test(NAME test_generators_combinators
    SOURCE test_generators_combinators.cpp
    USE_UTILS)
hegel_add_test(NAME test_generators_random
    SOURCE test_generators_random.cpp)
# Span labels derived from generator names and their components.
hegel_add_test(NAME test_labels SOURCE test_labels.cpp)
hegel_add_test(NAME test_stateful
    SOURCE test_stateful.cpp
    USE_UTILS
    APPROVALS)


# Explicit test cases: values given in advance, run before generation.
hegel_add_test(NAME test_explicit SOURCE test_explicit.cpp)

# Guiding generation: reject(), target(), repeat().
hegel_add_test(NAME test_guiding SOURCE test_guiding.cpp)

# Value printer for draw output.
hegel_add_test(NAME test_repr SOURCE test_repr.cpp)
hegel_add_test(NAME test_printer SOURCE test_printer.cpp)
hegel_add_test(NAME test_printable_generators
    SOURCE test_printable_generators.cpp)
# GoogleTest assertion macros inside a property body.
hegel_add_test(NAME test_gtest_macros SOURCE test_gtest_macros.cpp)
# Test-case cloning: clone(), spawn()/join(), and clone output approval.
hegel_add_test(NAME test_clone SOURCE test_clone.cpp USE_UTILS APPROVALS)

# default_generator is only available with reflection (C++20).
if(HEGEL_REFLECTION)
    hegel_add_test(NAME test_derived SOURCE test_derived.cpp)
endif()
add_executable(subject subject_main.cpp)
target_link_libraries(subject PRIVATE hegel)

# Printing / failure reporting: subject-subprocess output tests plus in-process
# per-verbosity diagnostics. Needs the subject binary for the output scenarios.
if(HEGEL_APPROVAL_TESTS)
    add_executable(test_printing test_printing.cpp common/approval_main.cpp)
    target_link_libraries(test_printing
        PRIVATE hegel_tests_common ApprovalTests::ApprovalTests)
    add_dependencies(test_printing subject)
    target_compile_definitions(test_printing
        PRIVATE
            HEGEL_SUBJECT_BIN="$<TARGET_FILE:subject>"
    )
    if(NOT HEGEL_THROW_SITE)
        target_compile_definitions(test_printing
            PRIVATE HEGEL_TESTS_NO_THROW_SITE)
    endif()
    gtest_discover_tests(test_printing)
endif()

add_executable(test_gtest test_gtest.cpp)
target_link_libraries(test_gtest
    PRIVATE
        hegel
        GTest::gtest
)

add_test(NAME test_gtest COMMAND test_gtest)
set_tests_properties(test_gtest PROPERTIES
    PASS_REGULAR_EXPRESSION "actual:"
)

# Misuses of printability that must not compile. Each case runs the compiler
# on one source and passes when the diagnostic matches.
set(HEGEL_COMPILE_FAIL_INCLUDES
    "$<TARGET_PROPERTY:hegel,INTERFACE_INCLUDE_DIRECTORIES>")
set(HEGEL_COMPILE_FAIL_SYSROOT "")
if(CMAKE_OSX_SYSROOT)
    set(HEGEL_COMPILE_FAIL_SYSROOT -isysroot ${CMAKE_OSX_SYSROOT})
endif()
set(HEGEL_COMPILE_FAIL_STANDARDS 17)
if(HEGEL_REFLECTION)
    list(APPEND HEGEL_COMPILE_FAIL_STANDARDS 20)
endif()
set(HEGEL_UNPRINTABLE_DRAW "tc.draw\\(\\) needs a printable generator")
set(HEGEL_COMPILE_FAIL_CASES
    "draw_any_generator|${HEGEL_UNPRINTABLE_DRAW}"
    "draw_map_to_opaque|${HEGEL_UNPRINTABLE_DRAW}"
    "print_as_call_on_silent_source|print_as_call\\(\\) needs a printable mapped generator"
    "print_as_repr_without_representation|print_as_repr\\(\\) needs a type with a C\\+\\+ representation"
    "return_silent_as_generator|[Cc]onver")
foreach(standard IN LISTS HEGEL_COMPILE_FAIL_STANDARDS)
    if(standard EQUAL 17)
        set(reflection 0)
    else()
        set(reflection 1)
    endif()
    foreach(case IN LISTS HEGEL_COMPILE_FAIL_CASES)
        string(REPLACE "|" ";" parts "${case}")
        list(GET parts 0 source)
        list(GET parts 1 diagnostic)
        add_test(NAME compile_fail_${source}_cxx${standard}
            COMMAND ${CMAKE_CXX_COMPILER} -fsyntax-only -std=c++${standard}
                ${HEGEL_COMPILE_FAIL_SYSROOT}
                -DHEGEL_HAS_REFLECTION=${reflection}
                "-I$<JOIN:${HEGEL_COMPILE_FAIL_INCLUDES},;-I>"
                ${CMAKE_CURRENT_SOURCE_DIR}/compile_fail/${source}.cpp
            COMMAND_EXPAND_LISTS)
        set_tests_properties(compile_fail_${source}_cxx${standard}
            PROPERTIES PASS_REGULAR_EXPRESSION "${diagnostic}")
    endforeach()
endforeach()
