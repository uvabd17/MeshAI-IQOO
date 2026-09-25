package ai.meshai.worker.core

/**
 * Pure wording for "is the model meshd wants already fully on this phone" (T098): LlamaRunner.ensureModel
 * checks this before it downloads a single byte of a file that is already sitting in modelsDir. No I/O
 * here — the runner decides whether a hash or a size actually matched; this only writes the note.
 */
object ModelVerify {
    enum class By { SHA256, SIZE }

    /** [expectedSha] is `Plan.model_sha` (proto/mesh.proto); blank means the plan carried none. */
    fun by(expectedSha: String): By = if (expectedSha.isNotBlank()) By.SHA256 else By.SIZE

    fun reusedNote(bytes: Long, by: By): String {
        val mb = CacheSummary.mb(bytes)
        val how = when (by) {
            By.SHA256 -> " (sha256 verified)"
            By.SIZE -> " (verified by size only — the plan carried no sha256)"
        }
        return "model already on this phone ($mb) — no download$how"
    }

    /** The coordinator could not be asked (network hiccup) or the file could not be hashed (I/O error):
     *  trust what is on disk rather than force a re-download, but say plainly that nothing was checked. */
    fun unverifiedNote(bytes: Long): String =
        "model already on this phone (${CacheSummary.mb(bytes)}) — no download (could not verify against the coordinator; keeping it)"

    fun mismatchNote(by: By): String = when (by) {
        By.SHA256 -> "cached model's sha256 does not match the plan — re-downloading"
        By.SIZE -> "cached model's size does not match the coordinator's copy — re-downloading"
    }
}
