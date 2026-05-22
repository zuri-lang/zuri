cmake_minimum_required(VERSION 3.18...3.26)

project(clib C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED True)

set(CMAKE_SHARED_LIBRARY_PREFIX "")

if(WIN32 OR MINGW)
    set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/zuri)
    set(ZURI_ROOT "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}")
else()
    set(ZURI_ROOT "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/zuri")
endif()

message(STATUS "Building clib module to ${ZURI_ROOT}/dist")

include_directories("${ZURI_ROOT}/includes" ${CMAKE_CURRENT_SOURCE_DIR})

add_library(clib SHARED clib.c)
target_link_libraries(clib libzuri)
add_dependencies(clib zuri)

find_package(PkgConfig)
find_path(FFI_INCLUDE_DIR ffi.h)
pkg_check_modules(LIBFFI REQUIRED IMPORTED_TARGET libffi)
target_link_libraries(clib PkgConfig::LIBFFI)

if(${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fPIC")
endif()

add_custom_target(copy-clib-to-libs ALL
        COMMAND ${CMAKE_COMMAND} -E copy_directory ${CMAKE_CURRENT_SOURCE_DIR}/clib ${ZURI_ROOT}/libs/clib
        COMMENT "Exporting clib source files to libs folder..."
        )

add_custom_command(TARGET clib POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:clib> ${ZURI_ROOT}/dist
        COMMENT "clib copy")