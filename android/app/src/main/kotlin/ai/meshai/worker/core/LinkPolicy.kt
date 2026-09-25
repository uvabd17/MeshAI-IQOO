package ai.meshai.worker.core

/**
 * ControlClient's decision when a new pairing payload arrives while already connected (JVM-tested in
 * LinkPolicyTest): a different mesh always wins; the same mesh only replaces the live link when the new
 * payload's best (first) host differs from the one we are on AND that host is confirmed reachable —
 * otherwise the working link is kept. Goal: one job at a time, most recent payload wins, no zombie sockets.
 */
object LinkPolicy {
    fun shouldReplaceLive(liveMeshId: String, liveHost: String, newMeshId: String, newFirstHost: String, newHostReachable: Boolean): Boolean {
        if (newMeshId != liveMeshId) return true
        if (newFirstHost == liveHost) return false
        return newHostReachable
    }
}
