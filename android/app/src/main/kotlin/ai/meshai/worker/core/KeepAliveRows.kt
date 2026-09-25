package ai.meshai.worker.core

/**
 * Maps the keep-alive booleans/ints Profiler reads from PowerManager/Settings into plain-word rows for
 * the "This phone" tab (T098) — pure so the wording is unit-testable without an Android framework.
 * Autostart (MIUI/HyperOS) has no public read API, so its row is always the same "go check" pointer.
 */
object KeepAliveRows {
    data class Row(val label: String, val value: String)

    /**
     * [batteryUnrestricted] null = PowerManager.isIgnoringBatteryOptimizations could not be read.
     * [screenTimeoutMin] non-positive (Settings.System.SCREEN_OFF_TIMEOUT missing or "never") reads as "never".
     */
    fun rows(batteryUnrestricted: Boolean?, stayOnWhilePluggedIn: Boolean, screenTimeoutMin: Int): List<Row> = listOf(
        Row("Battery", when (batteryUnrestricted) { true -> "not restricted"; false -> "restricted"; null -> "unknown" }),
        Row("Autostart", "cannot be read — open Settings"),
        Row("Keep screen on while plugged", if (stayOnWhilePluggedIn) "on" else "off"),
        Row("Screen timeout", if (screenTimeoutMin > 0) "$screenTimeoutMin min" else "never"),
    )
}
