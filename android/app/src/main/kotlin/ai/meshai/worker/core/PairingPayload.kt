package ai.meshai.worker.core

import org.json.JSONArray
import org.json.JSONObject

/**
 * The QR payload meshd shows: {"mesh_id","host","control_port","token","hosts":[…]}.
 * `hosts` lists every address of the laptop, best link first (USB tethering, ethernet, Wi-Fi); the
 * client tries them in turn so one QR works whichever link this phone is on.
 */
data class PairingPayload(val meshId: String, val host: String, val controlPort: Int, val token: String, val hosts: List<String> = listOf(host)) {
    val apiBase get() = "http://$host:8080"
    fun toJson(): String = JSONObject().put("mesh_id", meshId).put("host", host).put("control_port", controlPort).put("token", token).put("hosts", JSONArray(hosts)).toString()
    companion object {
        fun parse(s: String): PairingPayload? = runCatching {
            val j = JSONObject(s.trim())
            val host = j.getString("host")
            val hosts = j.optJSONArray("hosts")?.let { a -> (0 until a.length()).map { a.getString(it) } }?.filter { it.isNotBlank() } ?: emptyList()
            PairingPayload(j.getString("mesh_id"), host, j.optInt("control_port", 7070), j.getString("token"), if (hosts.isEmpty()) listOf(host) else (listOf(host) + hosts).distinct())
        }.getOrNull()
    }
}
