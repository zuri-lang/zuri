file(GLOB_RECURSE TEST_FILES "${CMAKE_SOURCE_DIR}/tests/*.zu")

FOREACH(file_path ${TEST_FILES})
    add_test(NAME ${file_path} COMMAND ${CMAKE_CURRENT_BINARY_DIR}/../zuri/${PROJECT_NAME} run ${file_path})
    message(STATUS "Adding test ${file_path}")
ENDFOREACH()