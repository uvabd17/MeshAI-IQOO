package ai.meshai.worker.core

import org.json.JSONObject

/** The QR payload meshd shows: {"mesh_id","host","control_port","token"}. */
data class PairingPayload(val meshId: String, val host: String, val controlPort: Int, val token: String) {
    val apiBase get() = "http://$host:8080"
    fun toJson(): String = JSONObject().put("mesh_id", meshId).put("host", host).put("control_port", controlPort).put("token", token).toString()
    companion object {
        fun parse(s: String): PairingPayload? = runCatching {
            val j = JSONObject(s.trim())
            PairingPayload(j.getString("mesh_id"), j.getString("host"), j.optInt("control_port", 7070), j.getString("token"))
        }.getOrNull()
    }
}
