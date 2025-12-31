
cmake_minimum_required(VERSION 3.28)
project(symgraph_demo LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_SCAN_FOR_MODULES ON)

add_library(foo)
# module interface unit
target_sources(foo PUBLIC FILE_SET CXX_MODULES FILES foo.cppm)

add_executable(main main.cpp)
target_link_libraries(main PRIVATE foo)
