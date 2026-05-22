cmake_minimum_required(VERSION 3.18...3.26)

project(sqlite C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED True)

set(CMAKE_SHARED_LIBRARY_PREFIX "")
set(ZURI_ROOT "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/zuri")

message(STATUS "Building SQLite module to ${ZURI_ROOT}/dist")

include_directories("${ZURI_ROOT}/includes" ${CMAKE_CURRENT_SOURCE_DIR})

include(FetchContent)
FetchContent_Declare(SQLite3
        URL      https://www.sqlite.org/2026/sqlite-amalgamation-3530000.zip
        URL_HASH SHA3_256=c2325c53b3b41761469f91cfb078e96882ac5d85bac10c11b0bd8f253b031e5b
        )
FetchContent_MakeAvailable(SQLite3)

add_library(sqlite SHARED "${sqlite3_SOURCE_DIR}/sqlite3.c" "${sqlite3_SOURCE_DIR}/sqlite3.h" sqlite.c)
target_include_directories(sqlite PRIVATE "${sqlite3_SOURCE_DIR}")
add_definitions(-DSQLITE_ENABLE_JSON1)
target_link_libraries(sqlite libzuri)
add_dependencies(sqlite zuri)

if(${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fPIC")
    target_link_libraries(sqlite pthread)
endif()

add_custom_target(copy-sqlite-to-libs ALL
        COMMAND ${CMAKE_COMMAND} -E copy_directory ${CMAKE_CURRENT_SOURCE_DIR}/sqlite ${ZURI_ROOT}/libs/sqlite
        COMMENT "Exporting sqlite source files to libs folder..."
        )

add_custom_command(TARGET sqlite POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:sqlite> ${ZURI_ROOT}/dist)