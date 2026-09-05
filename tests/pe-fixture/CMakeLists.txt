cmake_minimum_required(VERSION 3.20)
project(wisp_pe_fixture C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

find_program(CLANG clang REQUIRED)
find_program(LLD_LINK lld-link REQUIRED)

set(FIXTURE_OBJ "${CMAKE_CURRENT_BINARY_DIR}/wisp-test.obj")
set(FIXTURE_EXE "${CMAKE_CURRENT_BINARY_DIR}/wisp-test.exe")

add_custom_command(
    OUTPUT "${FIXTURE_OBJ}"
    COMMAND "${CLANG}"
        --target=x86_64-pc-windows-msvc
        -c "${CMAKE_CURRENT_SOURCE_DIR}/main.c"
        -o "${FIXTURE_OBJ}"
        -ffreestanding
        -fno-builtin
        -fno-stack-protector
    DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/main.c"
    VERBATIM
)

add_custom_command(
    OUTPUT "${FIXTURE_EXE}"
    COMMAND "${CMAKE_COMMAND}" -E make_directory "${CMAKE_CURRENT_BINARY_DIR}"
    COMMAND "${LLD_LINK}"
        /subsystem:console
        /entry:wisp_entry
        /nodefaultlib
        /machine:x64
        /out:wisp-test.exe
        wisp-test.obj
    DEPENDS "${FIXTURE_OBJ}"
    WORKING_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}"
    VERBATIM
)

add_custom_target(wisp-test ALL DEPENDS "${FIXTURE_EXE}")
