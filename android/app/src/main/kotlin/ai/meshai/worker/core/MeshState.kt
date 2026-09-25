package ai.meshai.worker.core

import ai.meshai.proto.Plan
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update

/** One device in the current plan, as the phone shows it (mirrors meshd's Mesh card). */
data class MeshPeer(val name: String, val kind: String, val spec: String, val role: String, val layerStart: Int, val layerEnd: Int, val bytes: Long, val isMe: Boolean, val reason: String)

data class UiState(
    val paired: Boolean = false,
    val connected: Boolean = false,
    val coordinator: String = "",
    val meshId: String = "",
    val role: String = "idle",            // idle | worker | host
    val planSummary: String = "",
    val layerStart: Int = 0,
    val layerEnd: Int = 0,
    val modelFile: String = "",
    val processRunning: Boolean = false,
    val downloadPct: Int = -1,
    val availBytes: Long = 0,
    val totalBytes: Long = 0,
    val thermalHeadroom: Float = 0f,
    val thermalStatus: Int = 0,
    val batteryPct: Int = 0,
    val charging: Boolean = false,
    val rttP50: Float = 0f,
    val rttP95: Float = 0f,
    val cpusAllowed: String = "?",
    val threads: Int = 0,
    val decodeTps: Float = 0f,
    val tier: String = "",
    val log: List<String> = emptyList(),
    val lastError: String? = null,
    /** A pairing payload that arrived from another app / adb and awaits the user's confirmation (M9). */
    val pendingJoin: PairingPayload? = null,
    /** Plain-language mesh view: model label, layer count and every device's share. */
    val modelLabel: String = "",
    val nLayer: Int = 0,
    val nCtx: Int = 0,
    val mesh: List<MeshPeer> = emptyList(),
    /** Per-core clock as a fraction of max (visual "cores at work"), and the capacity class of each core. */
    val coreLoads: List<Float> = emptyList(),
    val coreCaps: List<Int> = emptyList(),
    /** Last ~40 link RTT p50 samples for the sparkline. */
    val rttHistory: List<Float> = emptyList(),
    val socName: String = "",
    /** Setup stepper flags: specs sent to the laptop, worker confirmed listening. */
    val profileSent: Boolean = false,
    val workerReady: Boolean = false,
    val osName: String = "",
    /** RPC tensor cache (`-c`) on this phone right now (T098): what a resumed worker plan will reuse. */
    val rpcCacheBytes: Long = 0,
    val rpcCacheFiles: Int = 0,
    /** Keep-alive card (T098), read by Profiler.keepAlive(); null battery state = could not be read. */
    val batteryUnrestricted: Boolean? = null,
    val stayOnWhilePluggedIn: Boolean = false,
    val screenTimeoutMin: Int = 0,
)

object MeshState {
    val ui = MutableStateFlow(UiState())
    fun log(line: String) { android.util.Log.i("MeshAI", line); ui.update { it.copy(log = (it.log + line).takeLast(80)) } }
    fun set(f: (UiState) -> UiState) = ui.update(f)
    @Volatile var currentPlan: Plan? = null
}
