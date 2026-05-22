cmake_minimum_required(VERSION 3.18)

project(json C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED True)

set(CMAKE_SHARED_LIBRARY_PREFIX "")
set(ZURI_ROOT "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/zuri")

message(STATUS "Building JSON module to ${ZURI_ROOT}/dist")

include_directories("${ZURI_ROOT}/includes" ${CMAKE_CURRENT_SOURCE_DIR})

add_library(json SHARED json-parser.c json.h json.c)
target_link_libraries(json libzuri)
add_dependencies(json zuri)

if(${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fPIC")
endif()

if(WIN32 OR MINGW)
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -Wno-pointer-to-int-cast")
endif()

add_custom_target(copy-json-to-libs ALL
        COMMAND ${CMAKE_COMMAND} -E copy_directory ${CMAKE_CURRENT_SOURCE_DIR}/json ${ZURI_ROOT}/libs/json
        COMMENT "Exporting json source files to libs folder..."
        )

add_custom_command(TARGET json POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:json> ${ZURI_ROOT}/dist)