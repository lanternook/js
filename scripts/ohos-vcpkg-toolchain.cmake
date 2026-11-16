# CI-friendly: the OHOS native SDK location comes from the environment.
# Set OHOS_NATIVE_SDK to <sdk>/default/openharmony/native (or the Linux SDK
# native/ directory) before invoking vcpkg / cmake.
if(NOT DEFINED OHOS_NATIVE_SDK OR "${OHOS_NATIVE_SDK}" STREQUAL "")
  set(OHOS_NATIVE_SDK "$ENV{OHOS_NATIVE_SDK}")
endif()
if("${OHOS_NATIVE_SDK}" STREQUAL "")
  message(FATAL_ERROR "OHOS_NATIVE_SDK is not set; point it at the OHOS native SDK")
endif()

set(CMAKE_SYSTEM_NAME Linux)
set(CMAKE_SYSTEM_PROCESSOR aarch64)

set(CMAKE_SYSROOT "${OHOS_NATIVE_SDK}/sysroot")
# Use the repo wrappers (they embed --target/--sysroot/-D__MUSL__) so that
# configure/make-based ports (libvpx) also produce aarch64 objects; a bare
# clang path would default to the host architecture.
set(CMAKE_C_COMPILER "${CMAKE_CURRENT_LIST_DIR}/aarch64-unknown-linux-ohos-clang.sh")
set(CMAKE_CXX_COMPILER "${CMAKE_CURRENT_LIST_DIR}/aarch64-unknown-linux-ohos-clang++.sh")
set(CMAKE_C_COMPILER_TARGET aarch64-linux-ohos)
set(CMAKE_CXX_COMPILER_TARGET aarch64-linux-ohos)
set(CMAKE_AR "${OHOS_NATIVE_SDK}/llvm/bin/llvm-ar")
set(CMAKE_RANLIB "${OHOS_NATIVE_SDK}/llvm/bin/llvm-ranlib")
set(CMAKE_STRIP "${OHOS_NATIVE_SDK}/llvm/bin/llvm-strip")

set(CMAKE_C_FLAGS_INIT "-D__MUSL__")
set(CMAKE_CXX_FLAGS_INIT "-D__MUSL__")
set(CMAKE_FIND_ROOT_PATH "${CMAKE_SYSROOT}")
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)
