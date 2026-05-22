# define a function to simplify adding tests
function(add_zuri_test target arg index result)
    message(STATUS "setting up test ${arg}_${index} -> tests/${arg}.b")
    add_test(NAME ${arg}_${index} COMMAND ${CMAKE_CURRENT_BINARY_DIR}/../zuri/${PROJECT_NAME} ../zuri/tests/${arg}.b)
    set_tests_properties(${arg}_${index}
            PROPERTIES PASS_REGULAR_EXPRESSION ${result}
            )
endfunction(add_zuri_test)

file(GLOB_RECURSE TEST_FILES "${CMAKE_SOURCE_DIR}/tests/*.b")

FOREACH(file_path ${TEST_FILES})
    add_test(NAME ${file_path} COMMAND ${CMAKE_CURRENT_BINARY_DIR}/../zuri/${PROJECT_NAME} ${file_path})
    message(STATUS "Adding test ${file_path}")
ENDFOREACH()