cmake_minimum_required(VERSION 3.29)
project(arch-flux
    LANGUAGES C)

add_executable(arch-flux
    src/main.c)
add_executable(format-disk
    src/format_disk.c)

set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -Wall -Wextra \
    -Wformat -Wformat=2 -Wconversion -Wimplicit-fallthrough \
    -Werror=format-security \
    -fstrict-flex-arrays=3 \
    -fstack-clash-protection -fstack-protector-strong")

set(CMAKE_LINKER_TYPE MOLD)

set_target_properties(arch-flux PROPERTIES C_STANDARD 23
    C_STANDARD_REQUIRED ON
    C_EXTENSIONS OFF
    LINK_FLAGS "-Wl,-z,nodlopen -Wl,-z,noexecstack \
        -Wl,-z,relro -Wl,-z,now"
    LINK_FLAGS_RELEASE "-Wl,--strip-all"
    LINK_FLAGS_MINSIZEREL "-Wl,--strip-all")

target_compile_definitions(arch-flux PRIVATE U_FORTIFY_SOURCE D_FORTIFY_SOURCE=3 GLIBCXX_ASSERTIONS)