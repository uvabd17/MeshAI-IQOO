package ai.meshai.worker.core

/** What a phone-side stop must tell meshd (JVM-tested in StopReportTest). */
object StopReport {
    /**
     * @param had the (role, planId) of the process that was running, or null if none was
     * @param uiRoleBefore the UI role *before* the stop reset it ("host"/"worker" while a plan is being applied, e.g. mid-download)
     * @param applyingPlanId the id of the plan the actor is applying, or null
     * @return (role, planId) to report, or null when there is nothing meshd is waiting on
     */
    fun decide(had: Pair<String, String>?, uiRoleBefore: String, applyingPlanId: String?): Pair<String, String>? {
        if (had != null && had.second.isNotEmpty()) return had
        val planId = applyingPlanId ?: return null
        if (planId.isEmpty() || planId == "stop") return null
        if (uiRoleBefore != "host" && uiRoleBefore != "worker") return null
        return uiRoleBefore to planId
    }
}
