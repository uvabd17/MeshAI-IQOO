package ai.meshai.worker.core

/** Pure `/proc/<pid>/status` parsing (JVM-tested in ProcStatusTest); mirrors meshd's `parse_rss_anon`. */
object ProcStatus {
    /** RssAnon + RssShmem in bytes, or VmRSS − RssFile on kernels without the split fields; 0 if absent. */
    fun rssAnonBytes(status: String): Long {
        fun kb(key: String): Long? = status.lineSequence().firstOrNull { it.startsWith(key) }
            ?.split(Regex("\\s+"))?.getOrNull(1)?.toLongOrNull()
        val anon = kb("RssAnon:")
        return if (anon != null) (anon + (kb("RssShmem:") ?: 0L)) * 1024
        else ((kb("VmRSS:") ?: 0L) - (kb("RssFile:") ?: 0L)).coerceAtLeast(0L) * 1024
    }

    /** Android's UNIXProcess.toString() is "Process[pid=N, hasExited=false]"; 0 when it is anything else. */
    fun pidFromToString(s: String): Int = Regex("pid=(\\d+)").find(s)?.groupValues?.get(1)?.toIntOrNull() ?: 0
}
