package ai.meshai.worker.core

/**
 * Pure ownership logic for the one llama.cpp child process (JVM-tested in ProcessGateTest):
 *  - [arm] for a plan with a validity check (the link generation) that is re-evaluated under the lock at start time;
 *  - [tryStart] runs the spawn only if armed and still valid, and hands out a generation number;
 *  - [disarm] (stop, link loss, user) invalidates every earlier generation so a stale exit is never reported;
 *  - the plan id a process was started for is captured here, not read from mutable UI state later.
 */
class ProcessGate {
    private val lock = Any()
    private var armed = false
    private var valid: () -> Boolean = { true }
    private var gen = 0
    var currentRole = ""
        private set
    var currentPlanId = ""
        private set

    fun arm(valid: () -> Boolean) = synchronized(lock) { armed = true; this.valid = valid }

    /** Refused (null) if not armed or the validity check fails; otherwise the generation of the new process. */
    fun <T> tryStart(role: String, planId: String, spawn: () -> T): Pair<Int, T>? = synchronized(lock) {
        if (!armed || !valid()) return@synchronized null
        gen++ // any earlier process is stale from here on
        currentRole = role; currentPlanId = planId
        val v = spawn()
        gen to v
    }

    /**
     * Stop: disarms and invalidates the current generation, running [kill] under the same lock as [tryStart] so a
     * process that was just spawned can never be missed (round-7 #3). Returns the (role, planId) that was running.
     */
    fun disarm(kill: () -> Unit = {}): Pair<String, String>? = synchronized(lock) {
        armed = false; gen++
        val had = if (currentRole.isEmpty()) null else currentRole to currentPlanId
        currentRole = ""; currentPlanId = ""
        kill()
        had
    }

    /** A spawn that failed leaves nothing to own or report. */
    fun spawnFailed() = synchronized(lock) { currentRole = ""; currentPlanId = "" }

    fun isCurrent(g: Int) = synchronized(lock) { g == gen }
    val isArmed: Boolean get() = synchronized(lock) { armed }
}
