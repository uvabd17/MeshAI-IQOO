package ai.meshai.worker.core

/** Bytes and file count under a cache directory (LlamaRunner.rpcCacheStats reads these off disk). */
data class CacheStats(val bytes: Long, val files: Int)

/**
 * Pure formatting for "what is already on this phone from last time" (T098) — resuming a run should say
 * so instead of behaving as if nothing had happened. No I/O here: callers stat a directory or a file and
 * pass the numbers in, so the wording is unit-testable without an Android framework.
 */
object CacheSummary {
    /** "0 MB" below any real cache; whole MB (rounded up, so a few KB never reads as empty) below 1 GB; one-decimal GB at or above. */
    fun mb(bytes: Long): String = when {
        bytes <= 0L -> "0 MB"
        bytes >= 1_000_000_000L -> "%.1f GB".format(bytes / 1e9)
        else -> "${(bytes + 999_999) / 1_000_000} MB"
    }

    /** Appended to the worker's "listening:host:port" JobProgress note; empty when there is nothing cached yet. */
    fun jobProgressSuffix(bytes: Long, files: Int): String =
        if (bytes <= 0L || files <= 0) "" else " · cache: ${mb(bytes)} ($files file${if (files == 1) "" else "s"}) from last time"

    /** The Mesh tab's cache line; null when there is nothing cached yet (the caller skips the row). */
    fun meshTabLine(bytes: Long, files: Int): String? =
        if (bytes <= 0L || files <= 0) null else "Layers cached on this phone: ${mb(bytes)} ($files file${if (files == 1) "" else "s"})"
}
