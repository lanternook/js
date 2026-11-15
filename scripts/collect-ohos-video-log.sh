#!/usr/bin/env bash
set -euo pipefail

HDC="${HDC:-/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/hdc}"
TARGET="${HDC_TARGET:-127.0.0.1:5555}"

if [[ ! -x "$HDC" ]]; then
  echo "hdc not found: $HDC" >&2
  exit 1
fi

echo "Collecting RustDesk OHOS video logs from $TARGET (Ctrl-C to stop)..." >&2
exec "$HDC" -t "$TARGET" shell hilog -x \
  | rg --line-buffered -i 'RemoteRenderer|AVCodec|librustdesk|RemoteSession|video|frame|fps'
