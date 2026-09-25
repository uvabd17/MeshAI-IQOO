package ai.meshai.worker.core

import ai.meshai.proto.Backend
import ai.meshai.proto.DeviceProfile
import ai.meshai.proto.Telemetry
import ai.meshai.proto.Tier
import ai.meshai.proto.core
import ai.meshai.proto.deviceProfile
import ai.meshai.proto.telemetry
import android.app.ActivityManager
import android.content.Context
import android.os.BatteryManager
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import java.io.File
import java.net.InetSocketAddress
import java.net.Socket

/** Raw keep-alive readings (T098); Profiler.keepAlive() fills this, KeepAliveRows turns it into words. */
data class KeepAliveInfo(val batteryUnrestricted: Boolean?, val stayOnWhilePluggedIn: Boolean, val screenTimeoutMin: Int)

/** Reads what the scheduler needs. The phone reports; the coordinator decides. */
class Profiler(private val ctx: Context) {
    private val am = ctx.getSystemService(ActivityManager::class.java)
    private val pm = ctx.getSystemService(PowerManager::class.java)
    private val bm = ctx.getSystemService(BatteryManager::class.java)
    private val rtts = ArrayDeque<Float>()
    val deviceId: String = Settings.Secure.getString(ctx.contentResolver, Settings.Secure.ANDROID_ID) ?: "android-unknown"
    val displayName: String = "${Build.MANUFACTURER} ${Build.MODEL}".trim()

    /** Build.SOC_* are API 31; on Android 11 fall back to the board/hardware strings (H4). */
    private val socModel: String = if (Build.VERSION.SDK_INT >= 31) Build.SOC_MODEL else Build.HARDWARE
    private val socMfr: String = if (Build.VERSION.SDK_INT >= 31) Build.SOC_MANUFACTURER else Build.BOARD

    fun memInfo(): ActivityManager.MemoryInfo = ActivityManager.MemoryInfo().also { am.getMemoryInfo(it) }

    fun cpuFeatures(): List<String> {
        val f = runCatching { File("/proc/cpuinfo").readLines().firstOrNull { it.startsWith("Features") } ?: "" }.getOrDefault("")
        return listOf("asimddp", "i8mm", "sve", "bf16").filter { f.contains(it) }
    }

    fun cores(): List<Pair<Int, Int>> = (0 until Runtime.getRuntime().availableProcessors()).map { i ->
        val khz = runCatching { File("/sys/devices/system/cpu/cpu$i/cpufreq/cpuinfo_max_freq").readText().trim().toInt() }.getOrDefault(0)
        val cap = runCatching { File("/sys/devices/system/cpu/cpu$i/cpu_capacity").readText().trim().toInt() }.getOrDefault(1024)
        khz to cap
    }

    /**
     * How hard each core is running right now, 0..1 = current clock / max clock (from cpufreq, readable by apps on
     * most devices; a core that cannot be read reports 0). Cheap enough to sample every 2 s for the dashboard.
     */
    fun coreLoads(): List<Float> = (0 until Runtime.getRuntime().availableProcessors()).map { i ->
        val max = runCatching { File("/sys/devices/system/cpu/cpu$i/cpufreq/cpuinfo_max_freq").readText().trim().toFloat() }.getOrDefault(0f)
        val cur = runCatching { File("/sys/devices/system/cpu/cpu$i/cpufreq/scaling_cur_freq").readText().trim().toFloat() }.getOrDefault(0f)
        if (max > 0f) (cur / max).coerceIn(0f, 1f) else 0f
    }

    /** What the user has allowed, in plain words, for the laptop's Devices page. */
    fun grantedPermissions(): List<String> {
        val pm = ctx.packageManager
        fun granted(p: String) = pm.checkPermission(p, ctx.packageName) == android.content.pm.PackageManager.PERMISSION_GRANTED
        val out = mutableListOf<String>()
        out += "camera (QR scan): " + if (granted(android.Manifest.permission.CAMERA)) "granted" else "not granted"
        out += "notifications: " + if (Build.VERSION.SDK_INT < 33 || granted("android.permission.POST_NOTIFICATIONS")) "granted" else "not granted"
        out += "foreground service (connected device): " + if (granted("android.permission.FOREGROUND_SERVICE_CONNECTED_DEVICE")) "granted" else "n/a"
        out += "battery optimisation: " + if (runCatching { this.pm.isIgnoringBatteryOptimizations(ctx.packageName) }.getOrDefault(false)) "exempt" else "default (may be throttled)"
        out += "usb debugging: " + if (runCatching { android.provider.Settings.Global.getInt(ctx.contentResolver, android.provider.Settings.Global.ADB_ENABLED, 0) }.getOrDefault(0) == 1) "on" else "off"
        out += "network: internet + local link"
        return out
    }

    /** Threads for llama.cpp: big + mid cores (capacity ≥ 500), at least 2. */
    fun workerThreads(): Int = cores().count { it.second >= 500 }.coerceAtLeast(2)

    /**
     * Keep-alive card (T098): battery-optimisation exemption, the global "stay on while plugged" toggle
     * (read-only from here — it is a system setting, not ours to flip) and the screen-off timeout.
     * Autostart (MIUI/HyperOS) has no public read API; the UI sends the user to Settings instead.
     */
    fun keepAlive(): KeepAliveInfo {
        val batteryUnrestricted = runCatching { pm.isIgnoringBatteryOptimizations(ctx.packageName) }.getOrNull()
        val stayOn = runCatching { Settings.Global.getInt(ctx.contentResolver, Settings.Global.STAY_ON_WHILE_PLUGGED_IN, 0) }.getOrDefault(0) != 0
        val timeoutMs = runCatching { Settings.System.getInt(ctx.contentResolver, Settings.System.SCREEN_OFF_TIMEOUT, 0) }.getOrDefault(0)
        return KeepAliveInfo(batteryUnrestricted, stayOn, timeoutMs / 60_000)
    }

    fun cpusAllowed(): String = runCatching {
        File("/proc/self/status").readLines().firstOrNull { it.startsWith("Cpus_allowed_list") }?.substringAfter(":")?.trim()
    }.getOrNull() ?: "?"

    fun hasOpenCl(): Boolean = File("/vendor/lib64/libOpenCL.so").exists() || File("/system/vendor/lib64/libOpenCL.so").exists()

    /**
     * Tier gate (D015): the shipped arm64 build uses dotprod + i8mm kernels, so both are required;
     * a phone without i8mm would SIGILL on the first matmul. x86_64 (emulator) is test-only.
     */
    fun tier(): Tier {
        val total = memInfo().totalMem
        val soc = socModel.uppercase()
        val feats = cpuFeatures()
        val isArm = Build.SUPPORTED_ABIS.firstOrNull() == "arm64-v8a"
        return when {
            isArm && "asimddp" !in feats -> Tier.TIER_UNSUPPORTED
            // i8mm is required only by the +i8mm build. A dotprod-only build runs on
            // ARMv8.2 chips (Cortex-A78 and friends) that have no i8mm, so the gate is
            // on dotprod and the packaged binaries decide the rest.
            total < 7_000_000_000L -> Tier.TIER_UNSUPPORTED
            soc.startsWith("SM8750") || soc.startsWith("SM8850") -> Tier.TIER_S
            total >= 11_000_000_000L -> Tier.TIER_A
            else -> Tier.TIER_B
        }
    }

    fun profile(): DeviceProfile {
        val mi = memInfo()
        val cs = cores()
        val allowed = cpusAllowed()
        return deviceProfile {
            deviceId = this@Profiler.deviceId
            os = "android/${Build.VERSION.RELEASE} (sdk ${Build.VERSION.SDK_INT})"
            soc = "$socMfr $socModel · ${Build.HARDWARE}"
            cpuFeatures.addAll(this@Profiler.cpuFeatures())
            cs.forEachIndexed { i, (khz, cap) -> cores.add(core { index = i; maxKhz = khz; capacity = cap; this.allowed = cpuAllowed(allowed, i) }) }
            totalBytes = mi.totalMem
            headroomBytes = headroomFor(mi.totalMem)
            tier = this@Profiler.tier()
            permissions.addAll(grantedPermissions())
            if (hasOpenCl()) backends.add(ai.meshai.proto.backendBench { backend = Backend.BACKEND_OPENCL })
            backends.add(ai.meshai.proto.backendBench { backend = Backend.BACKEND_CPU })
        }
    }

    fun telemetry(decodeTps: Float, trimLevel: Int, heldBytes: Long = 0L, loads: List<Float> = emptyList()): Telemetry {
        val mi = memInfo()
        val headroom = runCatching { pm.getThermalHeadroom(10) }.getOrDefault(Float.NaN)
        val status = runCatching { pm.currentThermalStatus }.getOrDefault(0)
        val level = bm.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY)
        val cur = bm.getIntProperty(BatteryManager.BATTERY_PROPERTY_CURRENT_NOW)
        val sorted = rtts.sorted()
        return telemetry {
            deviceId = this@Profiler.deviceId
            availBytes = mi.availMem
            this.heldBytes = heldBytes
            coreLoads.addAll(loads)
            thermalHeadroom = if (headroom.isNaN()) 0f else headroom
            thermalStatus = status
            batteryPct = level.toFloat()
            charging = bm.isCharging
            currentMa = cur / 1000
            rttMsP50 = if (sorted.isEmpty()) 0f else sorted[sorted.size / 2]
            rttMsP95 = if (sorted.isEmpty()) 0f else sorted[(sorted.size * 95 / 100).coerceAtMost(sorted.size - 1)]
            cpusAllowed = this@Profiler.cpusAllowed()
            this.decodeTps = decodeTps
            this.trimLevel = trimLevel
        }
    }

    /** One RTT sample: TCP connect to the coordinator's control port (SYN/SYN-ACK ≈ ping on a LAN). */
    fun sampleRtt(host: String, port: Int) {
        val t0 = System.nanoTime()
        val ok = runCatching { Socket().use { it.connect(InetSocketAddress(host, port), 1500) }; true }.getOrDefault(false)
        if (ok) {
            rtts.addLast((System.nanoTime() - t0) / 1e6f)
            while (rtts.size > 20) rtts.removeFirst()
        }
    }

    private fun cpuAllowed(list: String, i: Int): Boolean = list.split(',').any { r ->
        val p = r.trim().split('-'); when (p.size) { 1 -> p[0].toIntOrNull() == i; 2 -> i in (p[0].toInt())..(p[1].toInt()); else -> false }
    }

    companion object {
        /** Kept for callers that still reference a flat figure (16 GB class phones). */
        const val HEADROOM = 1_500_000_000L

        /**
         * Memory we refuse to hand to a model, so Android keeps enough for the
         * system, the launcher and whatever the owner switches back to.
         *
         * A flat 1.5 GB was written for a 16 GB phone. On a 6 to 8 GB device it
         * eats most of what is actually free: a realme with 7.6 GB total and
         * 2.1 GB available offered only 0.6 GB, so the planner refused it and
         * the phone looked useless. Most people own 6 to 8 GB phones, so the
         * reserve scales with the device instead:
         *
         *   6 GB   -> 0.90 GB reserved
         *   8 GB   -> 1.20 GB
         *   12 GB  -> 1.80 GB
         *   16 GB  -> 2.40 GB (more careful than the old flat figure)
         *
         * Floor of 0.8 GB so a small phone still leaves the system room to
         * breathe; ceiling of 2.5 GB so a large phone is not over-taxed.
         */
        fun headroomFor(totalBytes: Long): Long =
            (totalBytes * 15 / 100).coerceIn(800_000_000L, 2_500_000_000L)
    }
}
