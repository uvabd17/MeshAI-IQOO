package ai.meshai.worker.core

import ai.meshai.proto.Plan
import ai.meshai.proto.envelope
import ai.meshai.proto.jobProgress
import ai.meshai.worker.MainActivity
import ai.meshai.worker.MeshApp
import ai.meshai.worker.R
import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.IBinder
import android.os.PowerManager
import ai.meshai.proto.Tier
import ai.meshai.proto.jobResult
import com.google.protobuf.kotlin.toByteString
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import java.net.InetSocketAddress
import java.net.Socket

/**
 * Foreground service (type connectedDevice — not time-capped, D005) that owns the control link,
 * the llama.cpp child process, a partial wake lock and a low-latency Wi-Fi lock while participating.
 * If the control link drops, the worker/host process is killed (H2): no orphaned RPC ports.
 */
class MeshService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private lateinit var profiler: Profiler
    private lateinit var runner: LlamaRunner
    private lateinit var client: ControlClient
    private var wake: PowerManager.WakeLock? = null
    private var wifi: WifiManager.WifiLock? = null
    /**
     * Plans are applied strictly in order by one actor; the current job is cancelled and joined before the next (N4).
     * Each queued plan carries the link generation it arrived on: after a link loss everything queued on the old
     * link is dropped, and link loss itself is a synthetic stop plan on the new generation (round-4 #3).
     */
    private class Queued(val plan: Plan, val linkGen: Int)
    private val plans = Channel<Queued>(Channel.UNLIMITED)
    private val linkGen = java.util.concurrent.atomic.AtomicInteger(0)
    /** Owned by the actor coroutine only. */
    private var planJob: Job? = null
    /** The newest real plan meshd sent, whether or not the actor has reached it yet (round-8 #4). */
    @Volatile private var lastReceivedPlan: Plan? = null

    override fun onCreate() {
        super.onCreate()
        profiler = Profiler(this)
        runner = LlamaRunner(this)
        client = ControlClient(this, scope, profiler, { if (it.planId != "stop") lastReceivedPlan = it; plans.send(Queued(it, linkGen.get())) }, ::onLinkLost)
        runner.onExit = { role, planId, code -> if (code != 0 && (role == "worker" || role == "host")) report(role, false, "$role process exited ($code)", planId) }
        // Actor: one plan at a time, in arrival order. A newer plan (or a stop) cancels and joins the running one
        // before it starts, so a stop can never be queued behind a long model download.
        scope.launch {
            for (q in plans) {
                if (q.linkGen != linkGen.get()) { MeshState.log("dropping plan ${q.plan.planId} queued on a lost link"); continue }
                planJob?.cancel(); planJob?.join()
                planJob = scope.launch { applyPlan(q.plan, q.linkGen) }
            }
        }
        MeshState.set { it.copy(threads = profiler.workerThreads(), cpusAllowed = profiler.cpusAllowed(), tier = profiler.tier().name.removePrefix("TIER_")) }
        if (!runner.available) MeshState.log("⚠ llama.cpp binaries missing from this build (jniLibs)")
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(1, notification("Ready to join a mesh"), ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)
        when (intent?.action) {
            ACTION_JOIN -> intent.getStringExtra(EXTRA_PAYLOAD)?.let { PairingPayload.parse(it) }?.let { p ->
                acquireLocks(); client.connect(p); MeshState.log("joining ${p.meshId} @ ${p.host}")
            } ?: MeshState.log("✗ invalid pairing payload")
            ACTION_LEAVE -> { linkGen.incrementAndGet(); runner.stop(); client.disconnect(); releaseLocks(); MeshState.set { it.copy(role = "idle", planSummary = "", modelFile = "") }; stopSelf() }
            ACTION_STOP_PROCESS -> userStop()
        }
        return START_STICKY
    }

    /** Tell the coordinator about a failed process or a refused plan (round-3 #2/L2). */
    private fun report(job: String, ok: Boolean, note: String, planId: String) =
        client.notify(envelope { jobResult = jobResult { jobId = job; this.ok = ok; output = note.toByteArray().toByteString(); this.planId = planId } })

    /**
     * Stop from the phone's own UI (round-6 #3): kill/disarm now, cancel the plan being applied (a host download
     * included) through the actor, and tell meshd — whether a process was running or the plan was still being
     * prepared — so it never waits on a phone that gave up.
     */
    private fun userStop() {
        val gen = linkGen.incrementAndGet() // an arm() that lands after this stop must not re-enable a start (round-7 #3)
        val uiRoleBefore = MeshState.ui.value.role // read BEFORE the reset (round-7 #1)
        val applying = MeshState.currentPlan?.takeIf { it.planId != "stop" }?.planId
        val had = runner.stop()
        plans.trySend(Queued(Plan.newBuilder().setPlanId("stop").build(), gen))
        MeshState.set { it.copy(role = "idle") }
        val reported = StopReport.decide(had, uiRoleBefore, applying)?.also { (role, planId) -> report(role, false, "$role stopped from the phone", planId) }
        // A newer plan that was queued (or being joined) behind the stopped one is dropped by the generation bump:
        // meshd must hear about that one too, or it waits for its bring-up timeout (round-8 #4).
        lastReceivedPlan?.takeIf { it.planId != reported?.second }?.let { p ->
            val me = p.placementsList.firstOrNull { it.deviceId == profiler.deviceId && it.used } ?: return@let
            report(if (me.isHost) "host" else "worker", false, "stopped from the phone before this plan was applied", p.planId)
        }
        lastReceivedPlan = null
    }

    /** Runs on the control client's IO thread: kill now (and disarm the runner), then let the actor settle. */
    private fun onLinkLost() {
        val gen = linkGen.incrementAndGet()
        MeshState.log("link lost — stopping llama.cpp process"); runner.stop()
        plans.trySend(Queued(Plan.newBuilder().setPlanId("stop").build(), gen))
        MeshState.set { it.copy(role = "idle") }
        updateNotification("Reconnecting…")
    }

    private suspend fun applyPlan(plan: Plan, gen: Int) {
        MeshState.currentPlan = plan
        val me = plan.placementsList.firstOrNull { it.deviceId == profiler.deviceId }
        if (plan.planId == "stop" || me == null || !me.used) {
            runner.stop()
            MeshState.set { it.copy(role = "idle", planSummary = if (plan.planId == "stop") "" else plan.summary, layerStart = 0, layerEnd = 0, modelFile = "") }
            updateNotification("Idle — paired")
            return
        }
        if (profiler.tier() == Tier.TIER_UNSUPPORTED) {
            MeshState.log("✗ refusing plan: this phone is below the floor (arm64 + dotprod + i8mm, ≥ 8 GB)")
            MeshState.set { it.copy(lastError = "unsupported device", role = "idle") }
            report("plan", false, "unsupported device (needs arm64 + dotprod + i8mm, ≥ 8 GB)", plan.planId)
            return
        }
        val threads = if (plan.nThreads > 0) plan.nThreads else profiler.workerThreads()
        val bind = client.linkLocalAddress ?: "127.0.0.1"
        runner.arm { linkGen.get() == gen } // a stop disarms; a link loss after this point makes the start refuse itself
        applyPlanInner(plan, me, threads, bind)
    }

    private suspend fun applyPlanInner(plan: Plan, me: ai.meshai.proto.Placement, threads: Int, bind: String) {
        MeshState.set { it.copy(planSummary = plan.summary, layerStart = me.layerStart, layerEnd = me.layerEnd, modelFile = plan.modelFile, threads = threads) }
        if (me.isHost) {
            MeshState.set { it.copy(role = "host") }
            updateNotification("Host: ${plan.modelFile}")
            try {
                val coord = plan.coordinator.ifEmpty { MeshState.ui.value.coordinator.substringBefore(':') + ":8080" }
                val file = runner.ensureModel("http://$coord", plan.modelFile)
                kotlinx.coroutines.currentCoroutineContext().ensureActive() // a stop/link loss during the download must not start the host
                val workers = plan.placementsList.filter { it.used && !it.isHost }
                val r = runner.startHost(bind, file, plan.nCtx, threads, workers.map { it.addr to it.rpcPort }, workers.map { it.layerEnd - it.layerStart }, plan.planId)
                if (r == LlamaRunner.Start.FAILED) report("host", false, "could not start llama-server", plan.planId) // a REFUSED start is a stop, not a failure
            } catch (e: kotlinx.coroutines.CancellationException) { throw e
            } catch (e: Exception) { MeshState.log("✗ host: ${e.message}"); MeshState.set { it.copy(lastError = e.message) }; report("host", false, e.message ?: "host start failed", plan.planId) }
        } else {
            MeshState.set { it.copy(role = "worker") }
            updateNotification("Worker: layers ${me.layerStart}–${me.layerEnd - 1}")
            val port = me.rpcPort.takeIf { it > 0 } ?: 50052
            kotlinx.coroutines.currentCoroutineContext().ensureActive()
            when (runner.startWorker(bind, port, threads, plan.planId)) {
                LlamaRunner.Start.STARTED -> reportListening(bind, port, plan.planId) // child of planJob: cancelled with it (round-5 #3)
                LlamaRunner.Start.FAILED -> report("worker", false, "could not start the RPC worker", plan.planId)
                LlamaRunner.Start.REFUSED -> {} // stopped or link lost meanwhile: nothing to report
            }
        }
    }

    /** Poll our own RPC port until it accepts, then tell the coordinator (with the plan id, M2). */
    private suspend fun reportListening(host: String, port: Int, planId: String) {
        repeat(60) {
            val ok = runCatching { Socket().use { it.connect(InetSocketAddress(host, port), 500) }; true }.getOrDefault(false)
            if (ok) {
                client.notify(envelope { jobProgress = jobProgress { jobId = "worker"; fraction = 1f; note = "listening:$host:$port"; this.planId = planId } })
                MeshState.log("worker listening on $host:$port")
                return
            }
            kotlinx.coroutines.delay(500)
        }
        MeshState.log("✗ worker did not start listening on $host:$port")
    }

    private fun acquireLocks() {
        releaseLocks()
        val pm = getSystemService(PowerManager::class.java)
        wake = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "meshai:worker").also { it.acquire(6 * 60 * 60 * 1000L) }
        val wm = applicationContext.getSystemService(WifiManager::class.java)
        wifi = wm.createWifiLock(WifiManager.WIFI_MODE_FULL_LOW_LATENCY, "meshai:lowlatency").also { it.acquire() }
    }
    private fun releaseLocks() { wake?.takeIf { it.isHeld }?.release(); wifi?.takeIf { it.isHeld }?.release(); wake = null; wifi = null }

    private fun notification(text: String): Notification {
        val pi = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        return Notification.Builder(this, MeshApp.CHANNEL).setSmallIcon(R.drawable.ic_launcher).setContentTitle("MeshAI").setContentText(text).setContentIntent(pi).setOngoing(true).build()
    }
    private fun updateNotification(text: String) = getSystemService(android.app.NotificationManager::class.java).notify(1, notification(text))

    override fun onTrimMemory(level: Int) { super.onTrimMemory(level); client.trimLevel = level; if (level >= 10 /* TRIM_MEMORY_RUNNING_LOW */) MeshState.log("⚠ onTrimMemory($level)") }
    override fun onDestroy() { runner.stop(); client.disconnect(); releaseLocks(); scope.cancel(); super.onDestroy() }
    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        const val ACTION_JOIN = "ai.meshai.JOIN"
        const val ACTION_LEAVE = "ai.meshai.LEAVE"
        const val ACTION_STOP_PROCESS = "ai.meshai.STOP_PROCESS"
        const val ACTION_BENCH = "ai.meshai.BENCH"
        const val EXTRA_PAYLOAD = "payload"
        fun join(ctx: Context, payload: String) = ctx.startForegroundService(Intent(ctx, MeshService::class.java).setAction(ACTION_JOIN).putExtra(EXTRA_PAYLOAD, payload))
        fun leave(ctx: Context) = ctx.startForegroundService(Intent(ctx, MeshService::class.java).setAction(ACTION_LEAVE))
        fun stopProcess(ctx: Context) = ctx.startForegroundService(Intent(ctx, MeshService::class.java).setAction(ACTION_STOP_PROCESS))
    }
}
