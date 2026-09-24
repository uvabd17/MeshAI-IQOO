package ai.meshai.worker.core

import ai.meshai.proto.Envelope
import ai.meshai.proto.Plan
import ai.meshai.proto.envelope
import ai.meshai.proto.hello
import android.content.Context
import com.google.protobuf.kotlin.toByteString
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.isActive as ctxIsActive
import java.io.DataInputStream
import java.io.DataOutputStream
import java.net.InetSocketAddress
import java.net.Socket

/**
 * One long-lived TCP stream to meshd: Hello(token | secret) → Profile → Telemetry every 2 s;
 * receives Paired / Plan / Heartbeat / Bye. Read timeout 45 s (heartbeats come every 10 s), so a
 * dead link is noticed and the worker is torn down (H2/M8). Plans are handled off the read loop
 * so a stop can arrive while a model is downloading (M7).
 */
class ControlClient(
    private val ctx: Context,
    private val scope: CoroutineScope,
    private val profiler: Profiler,
    private val onPlan: suspend (Plan) -> Unit,
    private val onLinkLost: () -> Unit,
    /** Anonymous RSS of this phone's llama.cpp child (D024 credit); the service wires LlamaRunner.heldBytes. */
    private val heldBytes: () -> Long = { 0L },
) {
    private val prefs = ctx.getSharedPreferences("meshai", Context.MODE_PRIVATE)
    private var job: Job? = null
    private var out: DataOutputStream? = null
    private var seq = 0L
    @Volatile var tokenOnce: String? = null
    @Volatile var decodeTps = 0f
    @Volatile var trimLevel = 0
    /** Address of our end of the control link — the RPC worker binds to exactly this (H2). */
    @Volatile var linkLocalAddress: String? = null

    private fun secretFor(meshId: String): ByteArray? = prefs.getString("secret:$meshId", null)?.let { hex -> hex.chunked(2).map { it.toInt(16).toByte() }.toByteArray() }
    private fun saveSecret(meshId: String, secret: ByteArray) = prefs.edit().putString("secret:$meshId", secret.joinToString("") { "%02x".format(it) }).apply()
    private fun clearSecret(meshId: String) = prefs.edit().remove("secret:$meshId").apply()

    fun connect(p: PairingPayload) {
        job?.cancel()
        tokenOnce = p.token
        MeshState.set { it.copy(coordinator = "${p.host}:${p.controlPort}", meshId = p.meshId, pendingJoin = null) }
        job = scope.launch(Dispatchers.IO) {
            var backoff = 1000L
            while (isActive) {
                var wasConnected = false
                try {
                    Socket().use { s ->
                        s.tcpNoDelay = true
                        s.connect(InetSocketAddress(p.host, p.controlPort), 4000)
                        s.soTimeout = 45_000 // coordinator heartbeats every 10 s; 45 s of silence = dead link
                        linkLocalAddress = s.localAddress.hostAddress
                        val din = DataInputStream(s.getInputStream().buffered())
                        val dout = DataOutputStream(s.getOutputStream().buffered())
                        out = dout
                        val tok = tokenOnce ?: ""
                        val secret = if (tok.isEmpty()) secretFor(p.meshId) else null
                        if (tok.isEmpty() && secret == null) { MeshState.log("✗ no pairing token and no stored secret — scan a new QR"); MeshState.set { it.copy(connected = false, paired = false, lastError = "not paired") }; return@launch }
                        send(envelope { hello = hello {
                            deviceId = profiler.deviceId; displayName = profiler.displayName
                            oneTimeToken = tok.toByteArray().toByteString(); rpcPort = 50052
                            if (secret != null) deviceSecret = secret.toByteString()
                        } })
                        val first = Framing.read(din)
                        if (first.hasBye()) {
                            MeshState.set { it.copy(connected = false, paired = false, lastError = first.bye.reason) }
                            MeshState.log("✗ ${first.bye.reason}")
                            tokenOnce = null; clearSecret(p.meshId)
                            return@launch
                        }
                        if (first.hasPaired()) { saveSecret(p.meshId, first.paired.deviceSecret.toByteArray()); MeshState.log("paired with ${first.paired.meshId}; secret stored") }
                        tokenOnce = null
                        wasConnected = true
                        MeshState.set { it.copy(connected = true, paired = true, lastError = null) }
                        MeshState.log("connected to ${p.host}:${p.controlPort} (our side ${linkLocalAddress})")
                        backoff = 1000L
                        send(envelope { profile = profiler.profile() })
                        val tele = launch {
                            while (isActive) {
                                profiler.sampleRtt(p.host, p.controlPort)
                                val loads = profiler.coreLoads()
                                val t = profiler.telemetry(if (decodeTps > 0f) decodeTps else MeshState.ui.value.decodeTps, trimLevel, heldBytes(), loads)
                                MeshState.set { it.copy(availBytes = t.availBytes, thermalHeadroom = t.thermalHeadroom, thermalStatus = t.thermalStatus, batteryPct = t.batteryPct.toInt(), charging = t.charging, rttP50 = t.rttMsP50, rttP95 = t.rttMsP95, cpusAllowed = t.cpusAllowed, totalBytes = profiler.memInfo().totalMem, decodeTps = maxOf(it.decodeTps, decodeTps), coreLoads = loads, rttHistory = (it.rttHistory + t.rttMsP50).takeLast(40)) }
                                runCatching { send(envelope { telemetry = t }) }
                                delay(2000)
                            }
                        }
                        try {
                            while (isActive) {
                                val env = Framing.read(din)
                                when {
                                    env.hasPlan() -> onPlan(env.plan) // hands off to the service's serial plan channel
                                    env.hasHeartbeat() -> {}
                                    env.hasPaired() -> saveSecret(p.meshId, env.paired.deviceSecret.toByteArray())
                                    env.hasBye() -> { MeshState.log("bye: ${env.bye.reason}"); break }
                                }
                            }
                        } finally { tele.cancel() }
                    }
                } catch (e: Exception) {
                    MeshState.set { it.copy(connected = false) }
                    MeshState.log("link: ${e.message ?: e.javaClass.simpleName}; retry in ${backoff / 1000}s")
                }
                out = null
                if (wasConnected) onLinkLost()
                delay(backoff); backoff = (backoff * 2).coerceAtMost(15000)
            }
        }
    }

    /** Fire-and-forget notification to the coordinator (e.g. worker listening). */
    fun notify(env: Envelope) = runCatching { send(env) }

    @Synchronized private fun send(env: Envelope) {
        val o = out ?: return
        Framing.write(o, env.toBuilder().setSeq(++seq).build())
    }

    fun disconnect() { job?.cancel(); job = null; out = null; MeshState.set { it.copy(connected = false, paired = false, role = "idle") } }
}
