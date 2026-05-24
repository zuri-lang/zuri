cmake_minimum_required(VERSION 3.18...3.26)

project(imagine C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED True)

set(CMAKE_SHARED_LIBRARY_PREFIX "")

if(WIN32 OR MINGW)
    set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/zuri)
    set(ZURI_ROOT "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}")
else()
    set(ZURI_ROOT "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/zuri")
endif()

message(STATUS "Building imagine module to ${ZURI_ROOT}/dist")

include_directories("${ZURI_ROOT}/includes" ${CMAKE_CURRENT_SOURCE_DIR})

add_library(imagine SHARED imagine.c)
target_link_libraries(imagine libzuri)
add_dependencies(imagine zuri)

find_package(PkgConfig)
pkg_check_modules(LIBGD REQUIRED IMPORTED_TARGET gdlib)
find_path(GD_INCLUDE_DIR gd.h)
target_include_directories(imagine PRIVATE ${GD_INCLUDE_DIR})
target_link_libraries(imagine PkgConfig::LIBGD)

if(${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fPIC")
endif()

add_custom_target(copy-imagine-to-libs ALL
        COMMAND ${CMAKE_COMMAND} -E copy_directory ${CMAKE_CURRENT_SOURCE_DIR}/imagine ${ZURI_ROOT}/libs/imagine
        COMMENT "Exporting imagine source files to libs folder..."
        )

add_custom_command(TARGET imagine POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:imagine> ${ZURI_ROOT}/dist
        COMMENT "imagine copy")