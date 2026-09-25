package ai.meshai.worker.core

import org.junit.Assert.assertEquals
import org.junit.Test

class KeepAliveRowsTest {
    private fun value(rows: List<KeepAliveRows.Row>, label: String) = rows.first { it.label == label }.value

    @Test
    fun batteryRowReflectsTheThreeStates() {
        assertEquals("not restricted", value(KeepAliveRows.rows(true, stayOnWhilePluggedIn = false, screenTimeoutMin = 1), "Battery"))
        assertEquals("restricted", value(KeepAliveRows.rows(false, stayOnWhilePluggedIn = false, screenTimeoutMin = 1), "Battery"))
        assertEquals("unknown", value(KeepAliveRows.rows(null, stayOnWhilePluggedIn = false, screenTimeoutMin = 1), "Battery"))
    }

    @Test
    fun autostartAlwaysPointsAtSettings() {
        // MIUI/HyperOS has no public read API for this — the row can never claim on/off.
        assertEquals("cannot be read — open Settings", value(KeepAliveRows.rows(true, stayOnWhilePluggedIn = true, screenTimeoutMin = 5), "Autostart"))
    }

    @Test
    fun stayOnWhilePluggedInIsOnOrOff() {
        assertEquals("on", value(KeepAliveRows.rows(null, stayOnWhilePluggedIn = true, screenTimeoutMin = 5), "Keep screen on while plugged"))
        assertEquals("off", value(KeepAliveRows.rows(null, stayOnWhilePluggedIn = false, screenTimeoutMin = 5), "Keep screen on while plugged"))
    }

    @Test
    fun screenTimeoutInMinutesOrNever() {
        assertEquals("5 min", value(KeepAliveRows.rows(null, stayOnWhilePluggedIn = false, screenTimeoutMin = 5), "Screen timeout"))
        assertEquals("never", value(KeepAliveRows.rows(null, stayOnWhilePluggedIn = false, screenTimeoutMin = 0), "Screen timeout"))
        assertEquals("never", value(KeepAliveRows.rows(null, stayOnWhilePluggedIn = false, screenTimeoutMin = -1), "Screen timeout"))
    }
}
