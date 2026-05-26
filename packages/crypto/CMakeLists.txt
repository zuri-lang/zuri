cmake_minimum_required(VERSION 3.18)

project(crypto C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED True)

set(CMAKE_SHARED_LIBRARY_PREFIX "")
set(ZURI_ROOT "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/zuri")

message(STATUS "Building crypto module to ${ZURI_ROOT}/dist")

include_directories("${ZURI_ROOT}/includes" ${CMAKE_CURRENT_SOURCE_DIR})

add_library(crypto SHARED crypto.c)
target_link_libraries(crypto libzuri)
add_dependencies(crypto zuri)

if(NOT ${CMAKE_SYSTEM_NAME} STREQUAL "Linux" AND UNIX)
    set(OPENSSL_USE_STATIC_LIBS TRUE)
endif()

find_package(OpenSSL REQUIRED)
if(NOT OPENSSL_FOUND)
    message(FATAL_ERROR "OpenSSL not found!")
else()
    include_directories(${OPENSSL_INCLUDE_DIR})
    target_link_libraries(crypto OpenSSL::Crypto)
endif()

if(UNIX)
    find_package(PkgConfig)
    pkg_check_modules(Argon2 REQUIRED IMPORTED_TARGET libargon2)
    find_path(LIBARGON2_INCLUDE_DIR argon2.h)
    target_include_directories(crypto PRIVATE ${LIBARGON2_INCLUDE_DIR})
    message(STATUS "Argon2 = ${LIBARGON2}")
    message(STATUS "Argon2 Includes = ${LIBARGON2_INCLUDE_DIR}")
    target_link_libraries(crypto PkgConfig::Argon2)
else()
    find_package(unofficial-argon2 CONFIG REQUIRED)
    target_link_libraries(crypto unofficial::argon2::libargon2)
endif()

if(${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -fPIC")
endif()

if(WIN32)
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -Wno-deprecated-declarations")
endif()

add_custom_target(copy-crypto-to-libs ALL
        COMMAND ${CMAKE_COMMAND} -E copy ${CMAKE_CURRENT_SOURCE_DIR}/crypto.zu ${ZURI_ROOT}/libs/crypto.zu
        COMMENT "Exporting crypto source files to libs folder..."
        )

add_custom_command(TARGET crypto POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:crypto> ${ZURI_ROOT}/dist)