cmake_minimum_required(VERSION 3.18)

project(hash C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED True)

set(CMAKE_SHARED_LIBRARY_PREFIX "")
set(ZURI_ROOT "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/zuri")

message(STATUS "Building hash module to ${ZURI_ROOT}/dist")

include_directories("${ZURI_ROOT}/includes" ${CMAKE_CURRENT_SOURCE_DIR})

add_library(hash SHARED fnv.h gost.h hash.c)
target_link_libraries(hash libzuri)
add_dependencies(hash zuri)

if(NOT ${CMAKE_SYSTEM_NAME} STREQUAL "Linux" AND UNIX)
    set(OPENSSL_USE_STATIC_LIBS TRUE)
endif()

find_package(OpenSSL REQUIRED)
if(NOT OPENSSL_FOUND)
    message(FATAL_ERROR "OpenSSL not found!")
else()
    include_directories(${OPENSSL_INCLUDE_DIR})
    target_link_libraries(hash OpenSSL::Crypto)
endif()

if(${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fPIC")
endif()

if(WIN32)
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -Wno-deprecated-declarations")
endif()

add_custom_target(copy-hash-to-libs ALL
        COMMAND ${CMAKE_COMMAND} -E copy ${CMAKE_CURRENT_SOURCE_DIR}/hash.b ${ZURI_ROOT}/libs/hash.b
        COMMENT "Exporting hash source files to libs folder..."
        )

add_custom_command(TARGET hash POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:hash> ${ZURI_ROOT}/dist)