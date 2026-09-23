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

/** Reads what the scheduler needs. The phone reports; the coordinator decides. */
class Profiler(private val ctx: Context) {
    private val am = ctx.getSystemService(ActivityManager::class.java)
    private val pm = ctx.getSystemService(PowerManager::class.java)
    private val bm = ctx.getSystemService(BatteryManager::class.java)
    private val rtts = ArrayDeque<Float>()
    val deviceId: String = Settings.Secure.getString(ctx.contentResolver, Settings.Secure.ANDROID_ID) ?: "android-unknown"
    val displayName: String = "${Build.MANUFACTURER} ${Build.MODEL}".trim()

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

    /** Threads for llama.cpp: big + mid cores (capacity ≥ 500), at least 2. */
    fun workerThreads(): Int = cores().count { it.second >= 500 }.coerceAtLeast(2)

    fun cpusAllowed(): String = runCatching {
        File("/proc/self/status").readLines().firstOrNull { it.startsWith("Cpus_allowed_list") }?.substringAfter(":")?.trim()
    }.getOrNull() ?: "?"

    fun hasOpenCl(): Boolean = File("/vendor/lib64/libOpenCL.so").exists() || File("/system/vendor/lib64/libOpenCL.so").exists()

    fun tier(): Tier {
        val total = memInfo().totalMem
        val soc = Build.SOC_MODEL.uppercase()
        val feats = cpuFeatures()
        return when {
            total < 7_000_000_000L || "asimddp" !in feats -> Tier.TIER_UNSUPPORTED
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
            soc = "${Build.SOC_MANUFACTURER} ${Build.SOC_MODEL} · ${Build.HARDWARE}"
            cpuFeatures.addAll(this@Profiler.cpuFeatures())
            cs.forEachIndexed { i, (khz, cap) -> cores.add(core { index = i; maxKhz = khz; capacity = cap; this.allowed = cpuAllowed(allowed, i) }) }
            totalBytes = mi.totalMem
            headroomBytes = HEADROOM
            tier = this@Profiler.tier()
            if (hasOpenCl()) backends.add(ai.meshai.proto.backendBench { backend = Backend.BACKEND_OPENCL })
            backends.add(ai.meshai.proto.backendBench { backend = Backend.BACKEND_CPU })
        }
    }

    fun telemetry(decodeTps: Float, trimLevel: Int): Telemetry {
        val mi = memInfo()
        val headroom = runCatching { pm.getThermalHeadroom(10) }.getOrDefault(Float.NaN)
        val status = runCatching { pm.currentThermalStatus }.getOrDefault(0)
        val level = bm.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY)
        val cur = bm.getIntProperty(BatteryManager.BATTERY_PROPERTY_CURRENT_NOW)
        val sorted = rtts.sorted()
        return telemetry {
            deviceId = this@Profiler.deviceId
            availBytes = mi.availMem
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

    /** One RTT sample: TCP connect to the coordinator (SYN/SYN-ACK), which tracks ping closely on a LAN. */
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

    companion object { const val HEADROOM = 1_500_000_000L }
}
