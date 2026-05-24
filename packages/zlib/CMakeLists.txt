cmake_minimum_required(VERSION 3.18)

project(zlib2 C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED True)

set(CMAKE_SHARED_LIBRARY_PREFIX "")

if(WIN32 OR MINGW)
    set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/zuri)
    set(ZURI_ROOT "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}")
else()
    set(ZURI_ROOT "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/zuri")
endif()

message(STATUS "Building zlib2 module to ${ZURI_ROOT}/dist")

include_directories("${ZURI_ROOT}/includes" ${CMAKE_CURRENT_SOURCE_DIR})

add_library(zlib2 SHARED zlib.c)
target_link_libraries(zlib2 libzuri)
add_dependencies(zlib2 zuri)

if(NOT ${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(ZLIB_USE_STATIC_LIBS ON)
endif()
include(FindZLIB)
if(NOT ZLIB_FOUND)
    message(STATUS "Can't find static zlib to link. Trying shared library...")
    set(ZLIB_USE_STATIC_LIBS OFF)
    include(FindZLIB)
    if(NOT ZLIB_FOUND)
        message(FATAL_ERROR "zlib not found!")
    endif()
else()
    include_directories(${ZLIB_INCLUDE_DIRS})
    target_link_libraries(zlib2 ${ZLIB_LIBRARIES})
endif()

if(${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fPIC")
endif()

add_custom_target(copy-zlib2-to-libs ALL
        COMMAND ${CMAKE_COMMAND} -E copy_directory ${CMAKE_CURRENT_SOURCE_DIR}/zlib ${ZURI_ROOT}/libs/zlib
        COMMENT "Exporting zlib source files to libs folder..."
        )

add_custom_command(TARGET zlib2 POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:zlib2> ${ZURI_ROOT}/dist)