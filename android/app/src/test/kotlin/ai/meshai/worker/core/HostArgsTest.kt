package ai.meshai.worker.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The phone-host split rule must equal meshd's `derive_args` (supervisor.rs tests use the same cases). */
class HostArgsTest {
    private fun after(args: List<String>, flag: String) = args.getOrNull(args.indexOf(flag) + 1)

    @Test
    fun splitOffloadsExactlyTheWorkerLayersAndPinsTheHead() {
        // Qwen3-0.6B: 28 layers; host 0-3, w1 4-14 (11), w2 15-27 (13). Verified against llama.cpp's
        // "layer N assigned to device" log on the laptop: CPU 0-3, RPC0 4-14, RPC1 15-27, head on CPU.
        val args = LlamaRunner.hostArgs("srv", "m.gguf", 2048, 4, "10.0.0.5", listOf("10.0.0.2" to 50052, "10.0.0.3" to 50052), listOf(11, 13))
        assertEquals("25", after(args, "-ngl")) // ngl+1: the output head counts as a layer
        assertEquals("^(output|output_norm|token_embd)\\.(weight|bias)$=CPU", after(args, "--override-tensor")) // same regex as meshd (D018/D033)
        assertEquals("off", after(args, "--fit")) // the planner's placement is final; llama.cpp must not re-fit it
        assertEquals("10.0.0.2:50052,10.0.0.3:50052", after(args, "--rpc"))
        assertEquals("0.440000,0.560000", after(args, "--tensor-split")) // 11/25 and (13+1)/25
        assertEquals("off", after(args, "--reasoning"))
        assertEquals("10.0.0.5", after(args, "--host"))
    }

    @Test
    fun singleWorkerHasNoTensorSplit() {
        val args = LlamaRunner.hostArgs("srv", "m.gguf", 4096, 6, "h", listOf("10.0.0.2" to 50052), listOf(20))
        assertEquals("21", after(args, "-ngl"))
        assertFalse(args.contains("--tensor-split"))
        assertEquals("off", after(args, "--fit"))
    }

    @Test
    fun noWorkersMeansNothingOffloaded() {
        val args = LlamaRunner.hostArgs("srv", "m.gguf", 4096, 6, "h", emptyList(), emptyList())
        assertEquals("0", after(args, "-ngl"))
        assertFalse(args.contains("--rpc"))
    }

    @Test
    fun splitUsesDotDecimalRegardlessOfLocale() {
        val prev = java.util.Locale.getDefault()
        java.util.Locale.setDefault(java.util.Locale.GERMANY)
        try {
            val args = LlamaRunner.hostArgs("srv", "m.gguf", 2048, 4, "h", listOf("a" to 1, "b" to 2), listOf(11, 13))
            assertTrue(after(args, "--tensor-split")!!.contains('.'))
            assertFalse(after(args, "--tensor-split")!!.contains(','.toString() + "4"))
        } finally { java.util.Locale.setDefault(prev) }
    }
}
