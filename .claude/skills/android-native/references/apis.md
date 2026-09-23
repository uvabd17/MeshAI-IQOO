# Android API reference for the MeshAI worker (checked 23 Sep 2026)

| Concern | API / mechanism | Min API | Notes |
|---|---|---|---|
| Cpuset visibility | `/proc/self/status` → `Cpus_allowed_list` | — | top-app = all cores; foreground = most; background = little cores ("pack"). Changes as the app moves between states. |
| Core ranking | `/sys/devices/system/cpu/cpuN/cpufreq/cpuinfo_max_freq`, `/sys/devices/system/cpu/cpuN/cpu_capacity` | — | Rank descending; SD 8 Elite Gen 5 = 2 prime + 6 performance (all "big"). |
| Thread affinity | `syscall(__NR_sched_setaffinity, gettid(), sizeof(cpu_set_t), &mask)` from NDK | — | Works only within the current cpuset; EINVAL if mask excludes all allowed CPUs. |
| Foreground service | `FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE`; permission `FOREGROUND_SERVICE_CONNECTED_DEVICE`; prerequisite e.g. `CHANGE_WIFI_STATE` | 34 (types mandatory) | Not time-capped. `dataSync`/`mediaProcessing`: 6 h / 24 h then `onTimeout(int,int)` on Android 15+. `specialUse` needs `android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE`. |
| Keep screen on | `window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)` | 1 | Only while the dashboard is the foreground Activity. |
| Wake lock | `PowerManager.newWakeLock(PARTIAL_WAKE_LOCK, tag)`; permission `WAKE_LOCK` | 1 | Acquire with timeout; release on plan end. |
| Wi-Fi latency | `WifiManager.createWifiLock(WIFI_MODE_FULL_LOW_LATENCY, tag)` | 29 | Effective only when app is foreground and screen on. |
| Thermal forecast | `PowerManager.getThermalHeadroom(forecastSeconds)` → float, 1.0 = severe throttling threshold | 30 | May return NaN when unsupported; treat NaN as unknown, not 0. |
| Thermal status | `PowerManager.addThermalStatusListener`, `getCurrentThermalStatus()` → `THERMAL_STATUS_NONE…SHUTDOWN` | 29 | Coarse; use for UI badge. |
| Battery | `BatteryManager.isCharging()`, `getIntProperty(BATTERY_PROPERTY_CAPACITY)`, `getIntProperty(BATTERY_PROPERTY_CURRENT_NOW)` (µA, sign varies by OEM) | 23 | Also `ACTION_POWER_CONNECTED` / `DISCONNECTED` broadcasts. |
| Memory | `ActivityManager.getMemoryInfo(MemoryInfo)` → `availMem`, `totalMem`, `threshold`, `lowMemory`; `ComponentCallbacks2.onTrimMemory(level)` | 16 | `TRIM_MEMORY_RUNNING_LOW` (10) / `_CRITICAL` (15) while foreground. |
| SoC / features | `Build.SOC_MODEL`, `Build.SOC_MANUFACTURER`; `/proc/cpuinfo` `Features:` → `asimddp` (dotprod), `i8mm`, `sve` | 31 for SOC_* | GenieX allow-list keys on SOC_MODEL (SM8750 = 8 Elite, SM8850 = 8 Elite Gen 5). |
| OpenCL presence | `dlopen("libOpenCL.so")` + `clGetPlatformIDs` succeeds | — | Adreno 7xx/8xx with recent drivers; llama.cpp OpenCL backend lists 750/810/830/840. |
| NPU | **NNAPI deprecated (Android 15).** Vendor path: Qualcomm GenieX (`com.qualcomm.qti:geniex-android`), BSD-3; Android 17 requires `FEATURE_NEURAL_PROCESSING_UNIT` in manifest for direct NPU use | — | Tier S only; whole-model-alone mode only (no multi-device). |
| Model file access | SAF `Intent.ACTION_OPEN_DOCUMENT` → `ContentResolver.openFileDescriptor(uri,"r")` → `ParcelFileDescriptor.getFd()` → path `/proc/self/fd/<fd>` | 19 | Avoids `MANAGE_EXTERNAL_STORAGE`. Take persistable URI permission for re-use. |
| App-private storage | `context.getExternalFilesDir(null)` | 8 | No permission; cleared on uninstall. |
| 16 KB pages | NDK r28+ default; check `llvm-readelf -l lib.so | grep LOAD` → `Align 0x4000` | Play: Nov 2025 | Applies to every bundled `.so`, including third-party. |
| ggml RPC in-process | `ggml_backend_rpc_start_server(const char* endpoint, const char* cache_dir, size_t n_threads, size_t n_devices, ggml_backend_dev_t* devices)` — blocking | — | Protocol v7.0.0, max 16 servers. Own thread. Explicit `devices[]`. |
| Threads for ggml | `n_threads` = big+mid core count; start 6 on 8 Elite Gen 5, measure 8 | — | `GGML_OPENMP=OFF` (pthreads) per llama.cpp Android doc. |
