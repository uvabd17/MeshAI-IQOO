package ai.meshai.worker.core

/**
 * MainActivity's decision to reconnect from the saved payload on launch (JVM-tested in AutoJoinTest).
 * A payload delivered by intent becomes a confirmation card instead (M9) and must never be raced by a
 * second job started from prefs; neither should a pending card already on screen, nor a mesh we are
 * already live on — starting a second ControlClient job used to leave the superseded one blocked in a
 * read until meshd dropped it 45 s later and killed the (healthy) new one's llama.cpp process with it.
 */
object AutoJoin {
    fun shouldAutoJoin(hasIntentPayload: Boolean, pendingJoinShown: Boolean, liveMeshId: String?, savedMeshId: String?): Boolean {
        if (hasIntentPayload || pendingJoinShown) return false
        if (savedMeshId.isNullOrEmpty()) return false
        if (liveMeshId != null && liveMeshId == savedMeshId) return false
        return true
    }
}
