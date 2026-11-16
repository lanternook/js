#!/bin/bash
# Diagnostic: report e_machine distribution of every .a member under
# $VCPKG_ROOT/installed/arm64-linux/lib. Non-fatal by design.
set +e
AR_BIN="${OHOS_NATIVE_SDK}/llvm/bin/llvm-ar"
LIBDIR="$VCPKG_ROOT/installed/arm64-linux/lib"
echo "DIAG AR=$AR_BIN LIBDIR=$LIBDIR"
if [ ! -x "$AR_BIN" ]; then echo "DIAG llvm-ar missing"; exit 0; fi
if [ ! -d "$LIBDIR" ]; then echo "DIAG no libdir"; exit 0; fi
for A in "$LIBDIR"/*.a; do
  N=$(basename "$A")
  FIRST=$("$AR_BIN" t "$A" 2>/dev/null | head -1)
  if [ -z "$FIRST" ]; then echo "DIAG $N unreadable"; continue; fi
  TDIR=$(mktemp -d)
  (cd "$TDIR" && "$AR_BIN" x "$A" 2>/dev/null)
  DIST=$(for F in "$TDIR"/*.o; do
           [ -f "$F" ] || continue
           od -An -tx1 -j18 -N2 "$F" 2>/dev/null
         done | sort | uniq -c | tr '\n' ';')
  echo "DIAG $N first=$FIRST dist=[$DIST]"
  rm -rf "$TDIR"
done
echo "DIAG DONE (b700=aarch64 3e00=x86_64 2800=arm32)"
exit 0
