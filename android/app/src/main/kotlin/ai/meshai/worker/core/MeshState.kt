package ai.meshai.worker.core

import ai.meshai.proto.Plan
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update

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
)

object MeshState {
    val ui = MutableStateFlow(UiState())
    fun log(line: String) = ui.update { it.copy(log = (it.log + line).takeLast(80)) }
    fun set(f: (UiState) -> UiState) = ui.update(f)
    @Volatile var currentPlan: Plan? = null
}
