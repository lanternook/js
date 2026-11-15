#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
UPSTREAM_DIR="${ROOT_DIR}/third_party/rustdesk"
TARGET="aarch64-unknown-linux-ohos"
OUTPUT_DIR="${ROOT_DIR}/rustdesk/libs/arm64-v8a"

export VCPKG_ROOT="${VCPKG_ROOT:-${ROOT_DIR}/third_party/vcpkg}"

OHOS_NATIVE_SDK="${OHOS_NATIVE_SDK:-${DEVECO_SDK_HOME:-/Applications/DevEco-Studio.app/Contents/sdk}/default/openharmony/native}"
export OHOS_NATIVE_SDK
export OHOS_SYSROOT="${OHOS_NATIVE_SDK}/sysroot"
export OHOS_CC="${ROOT_DIR}/scripts/aarch64-unknown-linux-ohos-clang.sh"
export OHOS_CXX="${ROOT_DIR}/scripts/aarch64-unknown-linux-ohos-clang++.sh"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_OHOS_LINKER="${OHOS_CC}"
export CC_aarch64_unknown_linux_ohos="${OHOS_CC}"
export CXX_aarch64_unknown_linux_ohos="${OHOS_CXX}"
export AR_aarch64_unknown_linux_ohos="${OHOS_NATIVE_SDK}/llvm/bin/llvm-ar"
export RANLIB_aarch64_unknown_linux_ohos="${OHOS_NATIVE_SDK}/llvm/bin/llvm-ranlib"
export BINDGEN_EXTRA_CLANG_ARGS_aarch64_unknown_linux_ohos="--target=aarch64-linux-ohos --sysroot=${OHOS_SYSROOT} -D__MUSL__"

if [[ ! -x "${OHOS_NATIVE_SDK}/llvm/bin/clang" ]]; then
  echo "HarmonyOS Native SDK not found: ${OHOS_NATIVE_SDK}" >&2
  echo "Set OHOS_NATIVE_SDK or DEVECO_SDK_HOME before building." >&2
  exit 1
fi

cd "${UPSTREAM_DIR}"
"${CARGO:-${HOME}/.cargo/bin/cargo}" build \
  --lib \
  --release \
  --target "${TARGET}" \
  --features flutter

mkdir -p "${OUTPUT_DIR}"
cp "${UPSTREAM_DIR}/target/${TARGET}/release/liblibrustdesk.so" \
  "${OUTPUT_DIR}/librustdesk.so"

PATCHELF="${PATCHELF:-$(command -v patchelf)}"
"${PATCHELF}" --set-soname librustdesk.so "${OUTPUT_DIR}/librustdesk.so"

echo "RustDesk OHOS core: ${OUTPUT_DIR}/librustdesk.so"
