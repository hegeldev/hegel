include(FetchContent)
FetchContent_Declare(
    SQLiteCpp
    GIT_REPOSITORY https://github.com/SRombauts/SQLiteCpp.git
    GIT_TAG 3.4.0
)
# Link the system SQLite, and skip SQLiteCpp's own lint steps. The system
# SQLite on macOS cannot load extensions.
set(SQLITECPP_INTERNAL_SQLITE OFF CACHE BOOL "" FORCE)
set(SQLITE_OMIT_LOAD_EXTENSION ON CACHE BOOL "" FORCE)
set(SQLITECPP_RUN_CPPLINT OFF CACHE BOOL "" FORCE)
set(SQLITECPP_RUN_CPPCHECK OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(SQLiteCpp)

add_executable(sqlite_bank bank.cpp)
target_link_libraries(sqlite_bank PRIVATE hegel SQLiteCpp)
