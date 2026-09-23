package ai.meshai.worker.core

import ai.meshai.proto.Envelope
import ai.meshai.proto.Plan
import ai.meshai.proto.envelope
import ai.meshai.proto.heartbeat
import ai.meshai.proto.hello
import com.google.protobuf.kotlin.toByteString
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.DataInputStream
import java.io.DataOutputStream
import java.net.InetSocketAddress
import java.net.Socket

/** One long-lived TCP stream to meshd: Hello → Profile → Telemetry every 2 s; receives Plan / Heartbeat / Bye. */
class ControlClient(
    private val scope: CoroutineScope,
    private val profiler: Profiler,
    private val onPlan: suspend (Plan) -> Unit,
) {
    private var job: Job? = null
    private var out: DataOutputStream? = null
    private var seq = 0L
    @Volatile var tokenOnce: String? = null
    @Volatile var decodeTps = 0f
    @Volatile var trimLevel = 0

    fun connect(p: PairingPayload) {
        job?.cancel()
        tokenOnce = p.token
        MeshState.set { it.copy(coordinator = "${p.host}:${p.controlPort}", meshId = p.meshId) }
        job = scope.launch(Dispatchers.IO) {
            var backoff = 1000L
            while (isActive) {
                try {
                    Socket().use { s ->
                        s.tcpNoDelay = true
                        s.connect(InetSocketAddress(p.host, p.controlPort), 4000)
                        val din = DataInputStream(s.getInputStream().buffered())
                        val dout = DataOutputStream(s.getOutputStream().buffered())
                        out = dout
                        val tok = tokenOnce ?: ""
                        send(envelope { hello = hello { deviceId = profiler.deviceId; displayName = profiler.displayName; oneTimeToken = tok.toByteArray().toByteString(); rpcPort = 50052 } })
                        val first = Framing.read(din)
                        if (first.hasBye()) { MeshState.set { it.copy(connected = false, paired = false, lastError = first.bye.reason) }; MeshState.log("✗ ${first.bye.reason}"); tokenOnce = null; return@launch }
                        tokenOnce = null // consumed; reconnects rely on being remembered by meshd
                        MeshState.set { it.copy(connected = true, paired = true, lastError = null) }
                        MeshState.log("connected to ${p.host}:${p.controlPort}")
                        backoff = 1000L
                        send(envelope { profile = profiler.profile() })
                        val tele = launch {
                            while (isActive) {
                                profiler.sampleRtt(p.host, 8080)
                                val t = profiler.telemetry(decodeTps, trimLevel)
                                MeshState.set { it.copy(availBytes = t.availBytes, thermalHeadroom = t.thermalHeadroom, thermalStatus = t.thermalStatus, batteryPct = t.batteryPct.toInt(), charging = t.charging, rttP50 = t.rttMsP50, rttP95 = t.rttMsP95, cpusAllowed = t.cpusAllowed, totalBytes = profiler.memInfo().totalMem) }
                                runCatching { send(envelope { telemetry = t }) }
                                delay(2000)
                            }
                        }
                        try {
                            while (isActive) {
                                val env = Framing.read(din)
                                when {
                                    env.hasPlan() -> withContext(Dispatchers.Default) { onPlan(env.plan) }
                                    env.hasHeartbeat() -> {}
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
                delay(backoff); backoff = (backoff * 2).coerceAtMost(15000)
            }
        }
    }

    @Synchronized private fun send(env: Envelope) {
        val o = out ?: return
        Framing.write(o, env.toBuilder().setSeq(++seq).build())
    }

    fun disconnect() { job?.cancel(); job = null; out = null; MeshState.set { it.copy(connected = false, paired = false, role = "idle") } }
}
