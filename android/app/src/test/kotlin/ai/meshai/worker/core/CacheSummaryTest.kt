package ai.meshai.worker.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class CacheSummaryTest {
    @Test
    fun zeroOrNegativeReadsAsZeroMb() {
        assertEquals("0 MB", CacheSummary.mb(0))
        assertEquals("0 MB", CacheSummary.mb(-5))
    }

    @Test
    fun aFewBytesRoundsUpRatherThanShowingZero() {
        assertEquals("1 MB", CacheSummary.mb(1))
        assertEquals("1 MB", CacheSummary.mb(500_000))
    }

    @Test
    fun wholeMegabytesBelowOneGigabyte() {
        assertEquals("234 MB", CacheSummary.mb(234_000_000))
        assertEquals("235 MB", CacheSummary.mb(234_500_001)) // rounds up on any remainder
    }

    @Test
    fun oneDecimalGigabytesAtOrAboveOneGigabyte() {
        assertEquals("1.0 GB", CacheSummary.mb(1_000_000_000))
        assertEquals("4.2 GB", CacheSummary.mb(4_200_000_000))
    }

    @Test
    fun jobProgressSuffixIsEmptyWithNothingCached() {
        assertEquals("", CacheSummary.jobProgressSuffix(0, 0))
        assertEquals("", CacheSummary.jobProgressSuffix(1_000, 0)) // bytes with no files is not a real cache
    }

    @Test
    fun jobProgressSuffixNamesSizeAndFileCount() {
        val suffix = CacheSummary.jobProgressSuffix(234_000_000, 12)
        assertEquals(" · cache: 234 MB (12 files) from last time", suffix)
    }

    @Test
    fun jobProgressSuffixSingularFile() {
        assertEquals(" · cache: 1 MB (1 file) from last time", CacheSummary.jobProgressSuffix(1, 1))
    }

    @Test
    fun meshTabLineNullWithNothingCached() {
        assertNull(CacheSummary.meshTabLine(0, 0))
    }

    @Test
    fun meshTabLineNamesSizeAndFileCount() {
        assertEquals("Layers cached on this phone: 234 MB (12 files)", CacheSummary.meshTabLine(234_000_000, 12))
    }
}
