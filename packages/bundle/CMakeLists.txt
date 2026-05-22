cmake_minimum_required(VERSION 3.18)

project(bundle C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED True)

set(ZURI_ROOT "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/zuri")
message(STATUS "Building Bundle support module to ${ZURI_ROOT}")

add_executable(bundle 
    bundle.c 
    "${CMAKE_CURRENT_SOURCE_DIR}/../../src/pathinfo.c" 
    "${CMAKE_CURRENT_SOURCE_DIR}/../../src/util.c"
)
include_directories("${CMAKE_CURRENT_SOURCE_DIR}/../../src/" "${INCLUDE_GEN_DIR}")


make_directory("${CMAKE_CURRENT_BINARY_DIR}/tmpbundle")

if(APPLE)
    add_custom_command(TARGET bundle POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy "${CMAKE_CURRENT_SOURCE_DIR}/macos/icon.icns" "${CMAKE_CURRENT_BINARY_DIR}/tmpbundle"
        COMMAND ${CMAKE_COMMAND} -E copy "${CMAKE_CURRENT_SOURCE_DIR}/macos/Info.plist" "${CMAKE_CURRENT_BINARY_DIR}/tmpbundle"
        COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:bundle> "${CMAKE_CURRENT_BINARY_DIR}/tmpbundle"
        COMMAND ${CMAKE_COMMAND} -E tar "cvf" "${CMAKE_CURRENT_BINARY_DIR}/bundle.zip" --format=zip .
        WORKING_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/tmpbundle"
        COMMENT "Creating default bundler at ${CMAKE_CURRENT_BINARY_DIR}/bundle.zip."
    )
elseif(LINUX OR WIN32)
    add_custom_command(TARGET bundle POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:bundle> "${CMAKE_CURRENT_BINARY_DIR}/tmpbundle"
        COMMAND ${CMAKE_COMMAND} -E tar "cvf" "${CMAKE_CURRENT_BINARY_DIR}/bundle.zip" --format=zip .
        WORKING_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/tmpbundle"
        COMMENT "Creating default bundler at ${CMAKE_CURRENT_BINARY_DIR}/bundle.zip."
    )
else()
    message(STATUS "Skipping Bundle support due to unsupported platform...")
endif()

add_custom_target(finalize-bundle-archive ALL
    COMMAND ${CMAKE_COMMAND} -E copy "${CMAKE_CURRENT_BINARY_DIR}/bundle.zip" ${ZURI_ROOT}/bundle.zip
    COMMENT "Exporting default bundler..."
)
add_dependencies(finalize-bundle-archive bundle)
