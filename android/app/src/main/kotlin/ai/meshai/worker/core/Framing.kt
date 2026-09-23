package ai.meshai.worker.core

import ai.meshai.proto.Envelope
import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.IOException

/** u32 big-endian length + Envelope bytes. Mirrors desktop/meshcore/src/framing.rs. */
object Framing {
    private const val MAX = 16 shl 20

    fun write(out: DataOutputStream, env: Envelope) {
        val b = env.toByteArray()
        out.writeInt(b.size)
        out.write(b)
        out.flush()
    }

    fun read(inp: DataInputStream): Envelope {
        val len = inp.readInt()
        if (len < 0 || len > MAX) throw IOException("frame too large: $len")
        val b = ByteArray(len)
        inp.readFully(b)
        return Envelope.parseFrom(b)
    }
}
