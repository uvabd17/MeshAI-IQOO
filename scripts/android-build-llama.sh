#!/usr/bin/env bash
# Cross-compile llama.cpp (ggml-rpc-server, llama-server, llama-bench, llama-cli)
# for Android arm64 with the NDK. Phase 0 step 1 — see docs/MESHAI.md (§10 design) §8.
#
# Usage:
#   scripts/android-build-llama.sh                 # CPU build (dotprod + i8mm)
#   scripts/android-build-llama.sh --opencl        # + Adreno OpenCL backend (needs OpenCL headers/lib, see below)
#   LLAMA_SRC=/path/to/llama.cpp scripts/android-build-llama.sh
#
# Verified 23 Sep 2026 on Ubuntu 25.10 with NDK 28.2.13676358 against llama.cpp master (ggml 0.25.0).
set -euo pipefail

NDK="${ANDROID_NDK_HOME:-$HOME/Android/Sdk/ndk/28.2.13676358}"   # r28+ => 16 KB page alignment by default
LLAMA_SRC="${LLAMA_SRC:-$(cd "$(dirname "$0")/.." && pwd)/third_party/llama.cpp}"
export MESHAI_MODELS="${MESHAI_MODELS:-/mnt/storage/meshai/models}"
ABI=arm64-v8a
PLATFORM=android-30                                                # minSdk 30: thermal headroom API
ARCH_FLAGS="-march=armv8.2-a+dotprod+i8mm"                         # capability floor: Tier B and up
OPENCL=0
[[ "${1:-}" == "--opencl" ]] && OPENCL=1

if [[ ! -d "$NDK" ]]; then echo "NDK not found at $NDK (set ANDROID_NDK_HOME)"; exit 1; fi
if [[ ! -d "$LLAMA_SRC" ]]; then
  echo "llama.cpp not found at $LLAMA_SRC — cloning upstream master (shallow)"
  git clone --depth 1 https://github.com/ggml-org/llama.cpp.git "$LLAMA_SRC"
fi

BUILD="$LLAMA_SRC/build-android-$ABI$([[ $OPENCL == 1 ]] && echo -opencl)"
EXTRA=()
if [[ $OPENCL == 1 ]]; then
  # OpenCL headers + ICD loader must be built for Android first:
  #   https://github.com/ggml-org/llama.cpp/blob/master/docs/backend/OPENCL.md  (Android section)
  # Set OPENCL_ROOT to the prefix that contains include/CL and lib/libOpenCL.so.
  : "${OPENCL_ROOT:?set OPENCL_ROOT to the Android OpenCL headers+loader prefix}"
  EXTRA+=(-DGGML_OPENCL=ON -DOpenCL_INCLUDE_DIR="$OPENCL_ROOT/include" -DOpenCL_LIBRARY="$OPENCL_ROOT/lib/libOpenCL.so")
fi

# Ninja is faster if present; fall back to Makefiles (verified path).
GEN="Unix Makefiles"; command -v ninja >/dev/null && GEN=Ninja

cmake -S "$LLAMA_SRC" -B "$BUILD" -G "$GEN" \
  -DCMAKE_TOOLCHAIN_FILE="$NDK/build/cmake/android.toolchain.cmake" \
  -DANDROID_ABI=$ABI -DANDROID_PLATFORM=$PLATFORM \
  -DCMAKE_BUILD_TYPE=Release \
  -DGGML_NATIVE=OFF -DGGML_OPENMP=OFF -DGGML_LLAMAFILE=OFF \
  -DLLAMA_OPENSSL=OFF -DLLAMA_CURL=OFF \
  -DGGML_RPC=ON -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF -DLLAMA_BUILD_TOOLS=ON \
  -DCMAKE_C_FLAGS="$ARCH_FLAGS" -DCMAKE_CXX_FLAGS="$ARCH_FLAGS" \
  "${EXTRA[@]}"

# NOTE: the RPC server target is `ggml-rpc-server` (binary name matches), not `rpc-server`.
cmake --build "$BUILD" --target ggml-rpc-server llama-server llama-bench llama-cli -j"$(nproc)"

echo
echo "== artifacts in $BUILD/bin =="
ls -la "$BUILD/bin"
READELF="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-readelf"
echo "== 16 KB page alignment (LOAD segment Align must be 0x4000) =="
"$READELF" -l "$BUILD/bin/ggml-rpc-server" | awk '/^\s+LOAD/ {print $NF}' | sort -u
