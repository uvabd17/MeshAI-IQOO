package ai.meshai.worker.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class StopReportTest {
    @Test
    fun runningProcessIsReportedWithItsOwnPlan() {
        assertEquals("worker" to "plan-1", StopReport.decide("worker" to "plan-1", "worker", "plan-2"))
    }

    @Test
    fun stopMidDownloadReportsThePlanBeingApplied() {
        // No process yet (had == null) but the host plan is being applied: meshd is waiting on us.
        assertEquals("host" to "plan-7", StopReport.decide(null, "host", "plan-7"))
    }

    @Test
    fun idlePhoneReportsNothing() {
        assertNull(StopReport.decide(null, "idle", null))
        assertNull(StopReport.decide(null, "idle", "plan-7"))
        assertNull(StopReport.decide(null, "host", "stop"))
        assertNull(StopReport.decide(null, "host", ""))
    }
}
