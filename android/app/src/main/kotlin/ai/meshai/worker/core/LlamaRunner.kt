package ai.meshai.worker.core

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.util.concurrent.TimeUnit

/**
 * Runs llama.cpp binaries shipped as jniLibs (D010): libmeshai_rpc.so (ggml-rpc-server),
 * libmeshai_server.so (llama-server), libmeshai_bench.so (llama-bench). They are extracted with exec
 * permission into nativeLibraryDir; LD_LIBRARY_PATH points there for libllama.so & co.
 */
class LlamaRunner(private val ctx: Context) {
    private val libDir = File(ctx.applicationInfo.nativeLibraryDir)
    private val lock = Any()
    private var proc: Process? = null
    /** Which run the current process belongs to, so an exit callback for an old process never masks a new one. */
    private var procGen = 0
    var onExit: ((role: String, code: Int) -> Unit)? = null
    @Volatile private var role = ""
    // Finite read timeout: a stalled model stream must not block the cancellable loop forever.
    private val http = OkHttpClient.Builder().readTimeout(60, TimeUnit.SECONDS).build()
    val modelsDir: File = File(ctx.getExternalFilesDir(null), "models").apply { mkdirs() }

    val available: Boolean get() = File(libDir, "libmeshai_rpc.so").exists()

    /** Worker: bind only to the address of the paired link (H2), never 0.0.0.0. */
    fun startWorker(bindHost: String, port: Int, threads: Int): Boolean =
        start("worker", listOf(File(libDir, "libmeshai_rpc.so").path, "-H", bindHost, "-p", "$port", "-t", "$threads", "-c"))

    /**
     * Same rule as desktop/meshd/src/supervisor.rs `derive_args`: llama.cpp counts the output head as a
     * layer, so offload ngl+1, pin the head to CPU, and split the (ngl+1) offloaded entries by worker
     * layer counts with the extra head slot on the last worker.
     */
    fun startHost(bindHost: String, model: File, nCtx: Int, threads: Int, workers: List<Pair<String, Int>>, workerLayers: List<Int>): Boolean {
        val ngl = workerLayers.sum()
        val args = mutableListOf(File(libDir, "libmeshai_server.so").path, "-m", model.path, "-c", "$nCtx", "-t", "$threads", "--host", bindHost, "--port", "8081", "--jinja", "--metrics", "--reasoning", "off")
        if (workers.isNotEmpty()) {
            args += listOf("--rpc", workers.joinToString(",") { "${it.first}:${it.second}" }, "-ngl", "${ngl + 1}", "--override-tensor", "output\\.weight=CPU")
            if (workers.size > 1) {
                val total = (ngl + 1).toDouble()
                val split = workerLayers.mapIndexed { i, n -> (if (i == workerLayers.lastIndex) n + 1 else n) / total }
                args += listOf("--tensor-split", split.joinToString(",") { String.format(java.util.Locale.ROOT, "%.6f", it) })
            }
        } else args += listOf("-ngl", "0")
        return start("host", args)
    }

    private fun start(newRole: String, cmd: List<String>): Boolean = synchronized(lock) {
        stopLocked()
        runCatching {
            val pb = ProcessBuilder(cmd).redirectErrorStream(true)
            pb.environment()["LD_LIBRARY_PATH"] = libDir.path
            pb.environment()["HOME"] = ctx.filesDir.path
            pb.directory(ctx.filesDir)
            val p = pb.start()
            proc = p; role = newRole
            val myGen = ++procGen
            MeshState.set { it.copy(processRunning = true, lastError = null) }
            MeshState.log("▶ ${cmd.drop(1).joinToString(" ")}")
            Thread {
                p.inputStream.bufferedReader().useLines { seq -> seq.forEach { MeshState.log(it.take(200)) } }
                val code = runCatching { p.waitFor() }.getOrDefault(-1)
                MeshState.log("■ $newRole process exited ($code)")
                val stillCurrent = synchronized(lock) { procGen == myGen }
                if (stillCurrent) { MeshState.set { it.copy(processRunning = false) }; onExit?.invoke(newRole, code) }
            }.start()
            true
        }.onFailure { MeshState.log("✗ start failed: ${it.message}"); MeshState.set { s -> s.copy(lastError = it.message) } }.getOrDefault(false)
    }

    fun stop() = synchronized(lock) { stopLocked() }

    private fun stopLocked() {
        procGen++ // any exit callback of the old process is now stale
        proc?.let { p -> runCatching { p.destroy(); if (!p.waitFor(2, TimeUnit.SECONDS)) p.destroyForcibly() } }
        proc = null
        MeshState.set { it.copy(processRunning = false) }
    }

    /** Fetch a model from the coordinator catalog if not cached. Resumable (M7). Returns the local file. */
    suspend fun ensureModel(coordinatorApi: String, file: String): File = withContext(Dispatchers.IO) {
        require(file.endsWith(".gguf") && !file.contains('/') && !file.contains("..")) { "bad model name" }
        val dest = File(modelsDir, file)
        if (dest.exists() && dest.length() > 0) return@withContext dest
        val part = File(modelsDir, "$file.part")
        val existing = if (part.exists()) part.length() else 0L
        val req = Request.Builder().url("$coordinatorApi/api/models/file/$file").apply { if (existing > 0) header("Range", "bytes=$existing-") }.build()
        http.newCall(req).execute().use { resp ->
            if (resp.code == 416) { part.delete(); error("resume rejected (416); partial file removed — retry") }
            if (!resp.isSuccessful) error("model fetch ${resp.code}")
            val resumed = resp.code == 206
            if (!resumed && existing > 0) part.delete()
            val total = resp.body!!.contentLength() + if (resumed) existing else 0L
            var got = if (resumed) existing else 0L
            var lastPct = -1
            resp.body!!.byteStream().use { inp ->
                java.io.FileOutputStream(part, resumed).use { out ->
                    val buf = ByteArray(1 shl 20)
                    while (true) {
                        if (!kotlinx.coroutines.currentCoroutineContext().isActive) error("download cancelled")
                        val n = inp.read(buf); if (n < 0) break
                        out.write(buf, 0, n); got += n
                        val pct = if (total > 0) (100 * got / total).toInt() else -1
                        if (pct != lastPct) { lastPct = pct; MeshState.set { it.copy(downloadPct = pct) } }
                    }
                }
            }
            if (total > 0 && got != total) error("download ended early ($got/$total) — will resume next time")
        }
        if (!part.renameTo(dest)) error("could not move ${part.name} into place")
        MeshState.set { it.copy(downloadPct = -1) }
        MeshState.log("model cached: $file (${dest.length() / 1_000_000} MB)")
        dest
    }

    /** Quick decode benchmark on a small model, used to fill Telemetry.decode_tps. */
    fun bench(model: File, threads: Int): Float = runCatching {
        val pb = ProcessBuilder(File(libDir, "libmeshai_bench.so").path, "-m", model.path, "-t", "$threads", "-p", "0", "-n", "32", "-r", "2", "-o", "json").redirectErrorStream(false)
        pb.environment()["LD_LIBRARY_PATH"] = libDir.path
        val p = pb.start()
        val out = p.inputStream.bufferedReader().readText()
        p.waitFor(180, TimeUnit.SECONDS)
        Regex("\"avg_ts\"\\s*:\\s*([0-9.]+)").findAll(out).map { it.groupValues[1].toFloat() }.lastOrNull() ?: 0f
    }.getOrDefault(0f)
}
