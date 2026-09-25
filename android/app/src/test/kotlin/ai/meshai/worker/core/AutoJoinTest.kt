package ai.meshai.worker.core

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AutoJoinTest {
    @Test
    fun intentPayloadWinsOverPrefs() {
        // offerFromIntent() already shows a confirmation card for this one; prefs must not race it.
        assertFalse(AutoJoin.shouldAutoJoin(hasIntentPayload = true, pendingJoinShown = false, liveMeshId = null, savedMeshId = "mesh-1"))
    }

    @Test
    fun pendingCardBlocksAutoJoin() {
        assertFalse(AutoJoin.shouldAutoJoin(hasIntentPayload = false, pendingJoinShown = true, liveMeshId = null, savedMeshId = "mesh-1"))
    }

    @Test
    fun noSavedPayloadNothingToJoin() {
        assertFalse(AutoJoin.shouldAutoJoin(hasIntentPayload = false, pendingJoinShown = false, liveMeshId = null, savedMeshId = null))
        assertFalse(AutoJoin.shouldAutoJoin(hasIntentPayload = false, pendingJoinShown = false, liveMeshId = null, savedMeshId = ""))
    }

    @Test
    fun alreadyLiveOnTheSameMeshIsNotJoinedAgain() {
        // A second job to a mesh we're already connected to is exactly what left a zombie socket before.
        assertFalse(AutoJoin.shouldAutoJoin(hasIntentPayload = false, pendingJoinShown = false, liveMeshId = "mesh-1", savedMeshId = "mesh-1"))
    }

    @Test
    fun liveOnADifferentMeshOrColdStartDoesAutoJoin() {
        assertTrue(AutoJoin.shouldAutoJoin(hasIntentPayload = false, pendingJoinShown = false, liveMeshId = null, savedMeshId = "mesh-1"))
        assertTrue(AutoJoin.shouldAutoJoin(hasIntentPayload = false, pendingJoinShown = false, liveMeshId = "mesh-2", savedMeshId = "mesh-1"))
    }
}
