cmake_minimum_required(VERSION 3.29)
project(arch-flux
    LANGUAGES CXX)

set(Boost_USE_STATIC_LIBS ON)
set(CMAKE_MAKE_PROGRAM ninja)
set(CMAKE_LINKER_TYPE MOLD)
find_package(Boost REQUIRED regex)
find_package(scn CONFIG REQUIRED)

add_executable(main-installer
    src/main.cc)
add_executable(format-disk
    src/format_disk.cc)
target_sources(format-disk PUBLIC FILE_SET CXX_MODULES FILES
    src/djb2a.ixx)
target_link_libraries(format-disk PRIVATE Boost::headers Boost::regex scn::scn)

set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -Wall -Wextra \
    -Wformat -Wformat=2 -Wconversion -Wimplicit-fallthrough \
    -Werror=format-security \
    -fstrict-flex-arrays=3 \
    -fstack-clash-protection -fstack-protector-strong")

set_target_properties(main-installer format-disk PROPERTIES
    CXX_STANDARD 23
    CXX_STANDARD_REQUIRED ON
    CXX_EXTENSIONS OFF
    LINK_FLAGS "-Wl,-z,nodlopen -Wl,-z,noexecstack \
        -Wl,-z,relro -Wl,-z,now \
        -static-libgcc -static-libstdc++"
    LINK_FLAGS_RELEASE "-Wl,--strip-all"
    LINK_FLAGS_MINSIZEREL "-Wl,--strip-all")

target_compile_definitions(main-installer PRIVATE U_FORTIFY_SOURCE D_FORTIFY_SOURCE=3 GLIBCXX_ASSERTIONS)
target_compile_definitions(format-disk PRIVATE U_FORTIFY_SOURCE D_FORTIFY_SOURCE=3 GLIBCXX_ASSERTIONS)