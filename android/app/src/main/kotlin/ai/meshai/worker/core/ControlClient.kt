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
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
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
 *
 * [connect] never leaves more than one job holding the socket: a superseded job's socket is closed
 * immediately (it may be blocked in the non-cancellable [Framing.read]), and a cancelled job's own
 * exit is never treated as a real link loss — otherwise a stale connection sat silent until meshd's
 * 45 s no-traffic timeout dropped it, and the *new*, healthy connection then took the blame.
 *
 * The decide → cancel-old → close-old-socket → launch-new sequence in [connect] runs under
 * [connectMutex]: [reachable] alone can block for up to 1.5 s, and without serialising the whole
 * sequence two overlapping [connect] calls (double-tap Join, a USB pairing re-sent while the first
 * is still deciding) could both read the same old [job], both cancel/close it, and each launch a
 * replacement — leaving one of the two new jobs unassigned to [job], i.e. a zombie link nothing
 * ever cancels. A later [connect] arriving mid-decide now queues on the mutex instead of racing it.
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
    /** Guards the decide → cancel-old → close-old-socket → launch-new sequence in [connect] (see class doc). */
    private val connectMutex = Mutex()
    @Volatile private var job: Job? = null
    private var out: DataOutputStream? = null
    /** The socket the live (or currently connecting) job owns; closed the instant it is superseded. */
    @Volatile private var socket: Socket? = null
    private var seq = 0L
    @Volatile var tokenOnce: String? = null
    @Volatile var decodeTps = 0f
    @Volatile var trimLevel = 0
    /** Address of our end of the control link — the RPC worker binds to exactly this (H2). */
    @Volatile var linkLocalAddress: String? = null

    private fun secretFor(meshId: String): ByteArray? = prefs.getString("secret:$meshId", null)?.let { hex -> hex.chunked(2).map { it.toInt(16).toByte() }.toByteArray() }
    private fun saveSecret(meshId: String, secret: ByteArray) = prefs.edit().putString("secret:$meshId", secret.joinToString("") { "%02x".format(it) }).apply()
    private fun clearSecret(meshId: String) = prefs.edit().remove("secret:$meshId").apply()

    /** Bounded TCP-connect reachability probe (mirrors Profiler.sampleRtt's own timeout), used only to
     *  decide whether a new payload's host is worth switching to (see [LinkPolicy]). */
    private fun reachable(host: String, port: Int): Boolean = runCatching { Socket().use { it.connect(InetSocketAddress(host, port), 1500) } }.isSuccess

    fun connect(p: PairingPayload) {
        tokenOnce = p.token
        MeshState.set { it.copy(pendingJoin = null) } // a join is being processed either way; the confirmation card is done
        scope.launch(Dispatchers.IO) {
            connectMutex.withLock {
                // Same mesh, already live: don't fight our own working link over a re-sent or stale payload
                // (round-trip re-pairing always targets the same host, e.g. 127.0.0.1 over USB) — LinkPolicy
                // keeps the current job unless the new payload's best host is both different and reachable.
                val ui = MeshState.ui.value
                if (job != null && ui.connected && ui.meshId == p.meshId) {
                    val liveHost = ui.coordinator.substringBefore(':')
                    val newHost = p.hosts.firstOrNull() ?: p.host
                    val reachableNow = newHost == liveHost || reachable(newHost, p.controlPort)
                    if (!LinkPolicy.shouldReplaceLive(ui.meshId, liveHost, p.meshId, newHost, reachableNow)) {
                        MeshState.log("already connected to ${p.meshId} @ $liveHost — keeping the live link")
                        return@withLock
                    }
                }
                job?.cancel()
                // The job we just cancelled may be blocked inside Framing.read (a blocking call cancellation
                // cannot interrupt); force its socket closed now so it never holds a silent link (D035).
                socket?.let { runCatching { it.close() } }
                socket = null
                out = null
                MeshState.set { it.copy(coordinator = "${p.host}:${p.controlPort}", meshId = p.meshId) }
                job = scope.launch(Dispatchers.IO) {
                    var backoff = 1000L
                    var attempt = 0
                    while (isActive) {
                        var wasConnected = false
                        // Try every address the laptop advertised, best link first; a working one is kept.
                        val host = p.hosts[attempt % p.hosts.size]; attempt++
                        val s = Socket()
                        socket = s
                        try {
                            s.use {
                                s.tcpNoDelay = true
                                s.connect(InetSocketAddress(host, p.controlPort), 4000)
                                MeshState.set { it.copy(coordinator = "$host:${p.controlPort}") }
                                attempt-- // stay on this host for the next reconnect
                                s.soTimeout = 45_000 // coordinator heartbeats every 10 s; 45 s of silence = dead link
                                linkLocalAddress = s.localAddress.hostAddress
                                val din = DataInputStream(s.getInputStream().buffered())
                                val dout = DataOutputStream(s.getOutputStream().buffered())
                                out = dout
                                val tok = tokenOnce ?: ""
                                val secret = secretFor(p.meshId) // always offered: meshd prefers a valid secret over a fresh token
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
                                    tokenOnce = null
                                    // Only a definitive "you are not paired / forgotten" invalidates our secret; a used-up
                                    // or expired token must not (that erased a valid secret once and deadlocked re-pairing).
                                    val r = first.bye.reason.lowercase()
                                    if (r.contains("not paired") || r.contains("forgotten") || r.contains("wrong device secret")) clearSecret(p.meshId)
                                    return@launch
                                }
                                if (first.hasPaired()) { saveSecret(p.meshId, first.paired.deviceSecret.toByteArray()); MeshState.log("paired with ${first.paired.meshId}; secret stored") }
                                prefs.edit().putString("last_payload", p.toJson()).apply() // so the app reconnects by itself next time
                                tokenOnce = null
                                wasConnected = true
                                MeshState.set { it.copy(connected = true, paired = true, lastError = null) }
                                MeshState.log("connected to ${p.host}:${p.controlPort} (our side ${linkLocalAddress})")
                                backoff = 1000L
                                send(envelope { profile = profiler.profile() })
                                MeshState.set { it.copy(profileSent = true) }
                                // RTT sampling is its own coroutine against the host we actually connected to (not the
                                // payload's stale primary host) and never delays a telemetry send if it blocks for its
                                // full 1.5 s timeout (e.g. the laptop's address changed underneath us).
                                val rtt = launch { while (isActive) { profiler.sampleRtt(host, p.controlPort); delay(2000) } }
                                val tele = launch {
                                    while (isActive) {
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
                                } finally { tele.cancel(); rtt.cancel() }
                            }
                        } catch (e: Exception) {
                            // isActive is false the instant this job was cancelled (a newer connect() superseded it);
                            // the exception here is just its own socket being force-closed above, not a real link
                            // event — the new job already owns the link, so retrying or reporting loss here would
                            // fight it (this used to stop llama.cpp on a phone that was, in fact, still connected).
                            if (!isActive) return@launch
                            MeshState.set { it.copy(connected = false) }
                            MeshState.log("link: ${e.message ?: e.javaClass.simpleName}; retry in ${backoff / 1000}s")
                        } finally {
                            if (socket === s) socket = null
                        }
                        out = null
                        if (wasConnected) onLinkLost()
                        delay(backoff); backoff = (backoff * 2).coerceAtMost(15000)
                    }
                }
            }
        }
    }

    /** Fire-and-forget notification to the coordinator (e.g. worker listening). */
    fun notify(env: Envelope) = runCatching { send(env) }

    @Synchronized private fun send(env: Envelope) {
        val o = out ?: return
        Framing.write(o, env.toBuilder().setSeq(++seq).build())
    }

    fun disconnect() {
        job?.cancel()
        socket?.let { runCatching { it.close() } }
        socket = null
        job = null
        out = null
        MeshState.set { it.copy(connected = false, paired = false, role = "idle") }
    }
}
