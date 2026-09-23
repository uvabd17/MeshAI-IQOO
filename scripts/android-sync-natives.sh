#!/usr/bin/env bash
# Copy llama.cpp Android builds into the app as jniLibs (D010). Executables are renamed lib*.so so the
# installer extracts them with exec permission; shared libs keep their names (resolved via LD_LIBRARY_PATH).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LLAMA="${LLAMA_SRC:-$ROOT/third_party/llama.cpp}"
STRIP_ARM="$HOME/Android/Sdk/ndk/28.2.13676358/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip"
for pair in "arm64-v8a:build-android-arm64-v8a" "x86_64:build-android-x86_64"; do
  abi="${pair%%:*}"; build="${pair##*:}"; src="$LLAMA/$build/bin"
  [[ -x "$src/ggml-rpc-server" && -x "$src/llama-server" && -x "$src/llama-bench" ]] || { echo "skip $abi ($src incomplete)"; continue; }
  dst="$ROOT/android/app/src/main/jniLibs/$abi"; mkdir -p "$dst"
  cp "$src/ggml-rpc-server" "$dst/libmeshai_rpc.so"
  cp "$src/llama-server"    "$dst/libmeshai_server.so"
  cp "$src/llama-bench"     "$dst/libmeshai_bench.so"
  cp "$src"/lib*.so "$dst/"
  # strip debug info: ~200 MB -> ~30 MB per ABI
  for f in "$dst"/*.so; do "$STRIP_ARM" --strip-unneeded "$f" 2>/dev/null || true; done
  echo "$abi: $(ls "$dst" | wc -l) files, $(du -sh "$dst" | cut -f1)"
done
