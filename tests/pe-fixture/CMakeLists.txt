cmake_minimum_required(VERSION 3.20)
project(wisp_pe_fixture C)

set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_C_COMPILER x86_64-w64-mingw32-gcc)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

add_executable(wisp-test main.c)

# Keep the first fixture deliberately tiny: no CRT startup, no DLL imports.
set_target_properties(wisp-test PROPERTIES
    OUTPUT_NAME "wisp-test"
    SUFFIX ".exe"
)

target_compile_options(wisp-test PRIVATE
    -ffreestanding
    -fno-builtin
    -fno-stack-protector
)

target_link_options(wisp-test PRIVATE
    -nostdlib
    -Wl,--entry,wisp_entry
    -Wl,--subsystem,console
)
