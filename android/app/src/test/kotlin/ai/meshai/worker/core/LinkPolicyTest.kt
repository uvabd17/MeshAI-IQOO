package ai.meshai.worker.core

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LinkPolicyTest {
    @Test
    fun differentMeshAlwaysReplaces() {
        assertTrue(LinkPolicy.shouldReplaceLive("mesh-1", "127.0.0.1", "mesh-2", "127.0.0.1", newHostReachable = false))
    }

    @Test
    fun sameMeshSameHostKeepsTheLiveLink() {
        // The USB re-pair case: every pairing targets 127.0.0.1 again — don't tear down a working link for it.
        assertFalse(LinkPolicy.shouldReplaceLive("mesh-1", "127.0.0.1", "mesh-1", "127.0.0.1", newHostReachable = true))
    }

    @Test
    fun sameMeshDifferentHostReplacesOnlyWhenReachable() {
        assertFalse(LinkPolicy.shouldReplaceLive("mesh-1", "192.168.21.175", "mesh-1", "192.168.1.50", newHostReachable = false))
        assertTrue(LinkPolicy.shouldReplaceLive("mesh-1", "192.168.21.175", "mesh-1", "192.168.1.50", newHostReachable = true))
    }
}
