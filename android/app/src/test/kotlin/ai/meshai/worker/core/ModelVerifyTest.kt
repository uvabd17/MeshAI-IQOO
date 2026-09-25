package ai.meshai.worker.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ModelVerifyTest {
    @Test
    fun aPlanShaPicksSha256Verification() {
        assertEquals(ModelVerify.By.SHA256, ModelVerify.by("abc123"))
    }

    @Test
    fun aBlankPlanShaFallsBackToSizeOnly() {
        assertEquals(ModelVerify.By.SIZE, ModelVerify.by(""))
    }

    @Test
    fun reusedNoteNamesHowItWasVerified() {
        assertEquals("model already on this phone (640 MB) — no download (sha256 verified)", ModelVerify.reusedNote(640_000_000, ModelVerify.By.SHA256))
        assertEquals(
            "model already on this phone (640 MB) — no download (verified by size only — the plan carried no sha256)",
            ModelVerify.reusedNote(640_000_000, ModelVerify.By.SIZE),
        )
    }

    @Test
    fun unverifiedNoteStillReusesButSaysSo() {
        val note = ModelVerify.unverifiedNote(640_000_000)
        assertTrue(note.startsWith("model already on this phone (640 MB) — no download"))
        assertTrue(note.contains("could not verify"))
    }

    @Test
    fun mismatchNoteNamesWhatFailed() {
        assertTrue(ModelVerify.mismatchNote(ModelVerify.By.SHA256).contains("sha256"))
        assertTrue(ModelVerify.mismatchNote(ModelVerify.By.SIZE).contains("size"))
    }
}
