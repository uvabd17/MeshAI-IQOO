package ai.meshai.worker.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Round-5/6 arm/refuse/report rules for the one child process, without a device. */
class ProcessGateTest {
    @Test
    fun unarmedStartIsRefused() {
        val g = ProcessGate()
        assertNull(g.tryStart("worker", "plan-1") { 1 })
    }

    @Test
    fun armedStartRunsAndCapturesRoleAndPlan() {
        val g = ProcessGate()
        g.arm { true }
        val r = g.tryStart("worker", "plan-1") { "proc" }
        assertNotNull(r)
        assertEquals("proc", r!!.second)
        assertEquals("worker", g.currentRole)
        assertEquals("plan-1", g.currentPlanId)
    }

    @Test
    fun linkLossAfterArmRefusesTheStart() {
        val g = ProcessGate()
        var linkGen = 1
        val mine = linkGen
        g.arm { linkGen == mine }
        linkGen++ // onLinkLost bumps the generation before disarming
        assertNull("a start after link loss must be refused even though arm() came later", g.tryStart("worker", "p") { 1 })
    }

    @Test
    fun stopDisarmsAndInvalidatesTheRunningGeneration() {
        val g = ProcessGate()
        g.arm { true }
        val (gen, _) = g.tryStart("host", "plan-2") { Unit }!!
        assertTrue(g.isCurrent(gen))
        val had = g.disarm()
        assertEquals("host" to "plan-2", had)
        assertFalse("a stale exit must not be reported", g.isCurrent(gen))
        assertFalse(g.isArmed)
        assertNull(g.tryStart("host", "plan-2") { Unit })
        assertEquals("", g.currentRole)
    }

    @Test
    fun aNewStartSupersedesTheOldGeneration() {
        val g = ProcessGate()
        g.arm { true }
        val (g1, _) = g.tryStart("worker", "plan-3") { Unit }!!
        val (g2, _) = g.tryStart("worker", "plan-4") { Unit }!!
        assertFalse(g.isCurrent(g1))
        assertTrue(g.isCurrent(g2))
        assertEquals("plan-4", g.currentPlanId)
    }

    @Test
    fun disarmWithNothingRunningReportsNothing() {
        assertNull(ProcessGate().disarm())
    }
}
