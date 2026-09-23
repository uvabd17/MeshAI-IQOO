package ai.meshai.worker.core

import org.junit.Assert.assertEquals
import org.junit.Test

class ProcStatusTest {
    @Test
    fun anonPlusShmem() {
        val t = "Name:\tllama-server\nVmRSS:\t   900000 kB\nRssAnon:\t  300000 kB\nRssFile:\t  590000 kB\nRssShmem:\t   10000 kB\n"
        assertEquals(310_000L * 1024, ProcStatus.rssAnonBytes(t))
    }

    @Test
    fun fallbackWithoutSplitFields() {
        assertEquals(310_000L * 1024, ProcStatus.rssAnonBytes("VmRSS:\t 900000 kB\nRssFile:\t 590000 kB\n"))
    }

    @Test
    fun pidFromProcessToString() {
        assertEquals(12345, ProcStatus.pidFromToString("Process[pid=12345, hasExited=false]"))
        assertEquals(0, ProcStatus.pidFromToString("java.lang.ProcessImpl@1a2b3c"))
    }

    @Test
    fun emptyIsZero() {
        assertEquals(0L, ProcStatus.rssAnonBytes(""))
    }
}
