package ai.meshai.worker.core

import android.content.Context
import kotlinx.coroutines.Dispatchers
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
    private var proc: Process? = null
    private val http = OkHttpClient.Builder().readTimeout(0, TimeUnit.MILLISECONDS).build()
    val modelsDir: File = File(ctx.getExternalFilesDir(null), "models").apply { mkdirs() }

    val available: Boolean get() = File(libDir, "libmeshai_rpc.so").exists()

    fun startWorker(port: Int, threads: Int): Boolean = start(listOf(File(libDir, "libmeshai_rpc.so").path, "-H", "0.0.0.0", "-p", "$port", "-t", "$threads", "-c"))

    fun startHost(model: File, nCtx: Int, threads: Int, workers: List<Pair<String, Int>>, nglLayers: Int, split: List<Double>): Boolean {
        val args = mutableListOf(File(libDir, "libmeshai_server.so").path, "-m", model.path, "-c", "$nCtx", "-t", "$threads", "--host", "0.0.0.0", "--port", "8081", "--jinja", "--metrics")
        if (workers.isNotEmpty()) {
            args += listOf("--rpc", workers.joinToString(",") { "${it.first}:${it.second}" }, "-ngl", "$nglLayers")
            if (workers.size > 1) args += listOf("--tensor-split", split.joinToString(",") { "%.4f".format(it) })
        } else args += listOf("-ngl", "0")
        return start(args)
    }

    private fun start(cmd: List<String>): Boolean {
        stop()
        return runCatching {
            val pb = ProcessBuilder(cmd).redirectErrorStream(true)
            pb.environment()["LD_LIBRARY_PATH"] = libDir.path
            pb.environment()["HOME"] = ctx.filesDir.path
            pb.directory(ctx.filesDir)
            val p = pb.start()
            proc = p
            MeshState.set { it.copy(processRunning = true, lastError = null) }
            MeshState.log("▶ ${cmd.drop(1).joinToString(" ")}")
            Thread {
                p.inputStream.bufferedReader().useLines { seq -> seq.forEach { MeshState.log(it.take(200)) } }
                val code = runCatching { p.waitFor() }.getOrDefault(-1)
                MeshState.log("■ process exited ($code)")
                MeshState.set { it.copy(processRunning = false) }
            }.start()
            true
        }.onFailure { MeshState.log("✗ start failed: ${it.message}"); MeshState.set { s -> s.copy(lastError = it.message) } }.getOrDefault(false)
    }

    fun stop() {
        proc?.let { p -> runCatching { p.destroy(); if (!p.waitFor(2, TimeUnit.SECONDS)) p.destroyForcibly() } }
        proc = null
        MeshState.set { it.copy(processRunning = false) }
    }

    /** Fetch a model from the coordinator catalog if not cached. Returns the local file. */
    suspend fun ensureModel(coordinatorApi: String, file: String): File = withContext(Dispatchers.IO) {
        val dest = File(modelsDir, file)
        if (dest.exists() && dest.length() > 0) return@withContext dest
        val part = File(modelsDir, "$file.part")
        val req = Request.Builder().url("$coordinatorApi/api/models/file/$file").build()
        http.newCall(req).execute().use { resp ->
            if (!resp.isSuccessful) error("model fetch ${resp.code}")
            val total = resp.body!!.contentLength()
            var got = 0L
            var lastPct = -1
            resp.body!!.byteStream().use { inp ->
                part.outputStream().use { out ->
                    val buf = ByteArray(1 shl 20)
                    while (true) {
                        val n = inp.read(buf); if (n < 0) break
                        out.write(buf, 0, n); got += n
                        val pct = if (total > 0) (100 * got / total).toInt() else -1
                        if (pct != lastPct) { lastPct = pct; MeshState.set { it.copy(downloadPct = pct) } }
                    }
                }
            }
        }
        part.renameTo(dest)
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
