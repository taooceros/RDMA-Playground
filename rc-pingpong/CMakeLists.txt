project("rc-pingpong")
add_executable(pingpong pingpong.c rc-pingpong.c)
target_link_libraries(pingpong ibverbs)