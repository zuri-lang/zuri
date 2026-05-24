cmake_minimum_required(VERSION 3.18)

project(curl C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED True)

set(CMAKE_SHARED_LIBRARY_PREFIX "")

if(WIN32 OR MINGW)
    set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/zuri)
    set(ZURI_ROOT "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}")
else()
    set(ZURI_ROOT "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/zuri")
endif()

message(STATUS "Building curl module to ${ZURI_ROOT}/dist")

include_directories("${ZURI_ROOT}/includes" ${CMAKE_CURRENT_SOURCE_DIR})

add_library(curl SHARED curl.c)
target_link_libraries(curl libzuri)
add_dependencies(curl zuri)

if(UNIX)
    target_link_libraries(curl m)
    include(FindCURL)
    if(${CURL_FOUND})
        include_directories(${CURL_INCLUDE_DIRS})
        target_link_libraries(curl ${CURL_LIBRARIES})
    else()
        message(FATAL_ERROR "cURL not found!")
    endif()
else()
    find_package(CURL REQUIRED)
    target_link_libraries(curl CURL::libcurl)
endif()

if(${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fPIC")
endif()

if(WIN32)
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -Wno-deprecated-declarations -Wno-attribute-warning")
endif()

add_custom_target(copy-curl-to-libs ALL
        COMMAND ${CMAKE_COMMAND} -E copy_directory ${CMAKE_CURRENT_SOURCE_DIR}/curl ${ZURI_ROOT}/libs/curl
        COMMENT "Exporting curl source files to libs folder..."
        )

add_custom_command(TARGET curl POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:curl> ${ZURI_ROOT}/dist)