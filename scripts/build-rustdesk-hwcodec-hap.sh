#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEVECO_SDK_HOME="${DEVECO_SDK_HOME:-/Applications/DevEco-Studio.app/Contents/sdk}"
NODE_HOME="${NODE_HOME:-/Applications/DevEco-Studio.app/Contents/tools/node}"
HVIGOR="${HVIGOR:-/Applications/DevEco-Studio.app/Contents/tools/hvigor/bin/hvigorw}"

if [[ ! -x "$HVIGOR" ]]; then
  echo "Hvigor not found: $HVIGOR" >&2
  exit 1
fi

export DEVECO_SDK_HOME NODE_HOME RUSTDESK_OHOS_HWCODEC=ON
cd "$PROJECT_ROOT"
exec "$HVIGOR" --mode module -p product=default -p module=rustdesk@default \
  -p buildMode=debug assembleHap --no-daemon
