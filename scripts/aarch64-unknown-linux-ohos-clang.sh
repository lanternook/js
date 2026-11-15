#!/bin/sh
SDK_NATIVE="${OHOS_NATIVE_SDK:-/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/native}"
exec "$SDK_NATIVE/llvm/bin/clang" \
  -target aarch64-linux-ohos \
  --sysroot="$SDK_NATIVE/sysroot" \
  -D__MUSL__ \
  "$@"
