package ai.meshai.worker.core

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.util.concurrent.TimeUnit

/**
 * Runs llama.cpp binaries shipped as jniLibs (D010): libmeshai_rpc.so (ggml-rpc-server),
 * libmeshai_server.so (llama-server), libmeshai_bench.so (llama-bench). They are extracted with exec
 * permission into nativeLibraryDir; LD_LIBRARY_PATH points there for libllama.so & co.
 *
 * Tensor cache (T098): `-c`/`--cache` in rpc-server.cpp is a bare flag with no path argument — it only
 * turns caching on. The directory itself is resolved *inside* the RPC server from `LLAMA_CACHE`, else
 * `XDG_CACHE_HOME`, else `$HOME/.cache/llama.cpp` (`fs_get_cache_directory()`), and `main()` always
 * appends `rpc/` to whatever that resolves to (third_party/llama.cpp/tools/rpc/rpc-server.cpp:127-171,
 * 235-236, 319-328). So the only way to put it somewhere explicit is the environment, not an argv flag:
 * we pin `LLAMA_CACHE` to [rpcCacheRoot] (inside the app's own files dir, survives restarts, cleared on
 * uninstall) and the server ends up writing under [rpcCacheDir] (`rpcCacheRoot/rpc/`).
 */
class LlamaRunner(private val ctx: Context) {
    private val libDir = File(ctx.applicationInfo.nativeLibraryDir)
    private val gate = ProcessGate()
    /** Assigned and killed only under the gate lock (round-7 #3). */
    @Volatile private var proc: Process? = null
    /** (role, planId the process was started for, exit code) — only for the process we still own. */
    var onExit: ((role: String, planId: String, code: Int) -> Unit)? = null
    /** Model download progress 0..100, invoked on every whole-percent change (the service relays it to meshd). */
    var onDownload: ((pct: Int) -> Unit)? = null
    enum class Start { STARTED, REFUSED, FAILED }
    val currentRole: String get() = gate.currentRole
    /** True while our child process is alive. */
    val isRunning: Boolean get() = proc?.let { p -> runCatching { p.isAlive }.getOrDefault(false) } ?: false
    val currentPlanId: String get() = gate.currentPlanId
    // Finite read timeout: a stalled model stream must not block the cancellable loop forever.
    private val http = OkHttpClient.Builder().readTimeout(60, TimeUnit.SECONDS).build()
    val modelsDir: File = File(ctx.getExternalFilesDir(null), "models").apply { mkdirs() }
    /** What we point `LLAMA_CACHE` at; see the class doc for why this isn't a `-c <dir>` argument. */
    val rpcCacheRoot: File = File(ctx.filesDir, "rpc-cache")
    /** Where the RPC server actually writes (it appends "rpc/" to LLAMA_CACHE itself). */
    val rpcCacheDir: File = File(rpcCacheRoot, "rpc")

    val available: Boolean get() = File(libDir, "libmeshai_rpc.so").exists()

    /** Arm for one plan; [valid] (the link generation) is re-checked under the lock at start time (round-5 #3). */
    fun arm(valid: () -> Boolean) = gate.arm(valid)

    /** Worker: bind only to the address of the paired link (H2), never 0.0.0.0. */
    fun startWorker(bindHost: String, port: Int, threads: Int, planId: String): Start =
        start("worker", planId, listOf(File(libDir, "libmeshai_rpc.so").path, "-H", bindHost, "-p", "$port", "-t", "$threads", "-c"))

    /** Bytes and file count under [rpcCacheDir] right now — cheap enough to call before every worker start. */
    fun rpcCacheStats(): CacheStats {
        if (!rpcCacheDir.isDirectory) return CacheStats(0L, 0)
        var bytes = 0L; var files = 0
        rpcCacheDir.walkTopDown().forEach { f -> if (f.isFile) { bytes += f.length(); files++ } }
        return CacheStats(bytes, files)
    }

    fun startHost(bindHost: String, model: File, nCtx: Int, threads: Int, workers: List<Pair<String, Int>>, workerLayers: List<Int>, planId: String): Start =
        start("host", planId, hostArgs(File(libDir, "libmeshai_server.so").path, model.path, nCtx, threads, bindHost, workers, workerLayers))

    private fun start(newRole: String, planId: String, cmd: List<String>): Start {
        var spawnError: Throwable? = null
        val started = gate.tryStart(newRole, planId) {
            killProcess() // the previous process, if any, is ours to replace
            runCatching {
                val pb = ProcessBuilder(cmd).redirectErrorStream(true)
                pb.environment()["LD_LIBRARY_PATH"] = libDir.path
                pb.environment()["HOME"] = ctx.filesDir.path
                pb.environment()["LLAMA_CACHE"] = rpcCacheRoot.path // worker's `-c`; see the class doc
                pb.directory(ctx.filesDir)
                pb.start().also { proc = it } // published under the lock: a concurrent stop sees it
            }.onFailure { spawnError = it }.getOrNull() // null → the gate clears the owner under the lock
        } ?: run { MeshState.log("✗ $newRole start refused: stopped (or link lost) before it could begin"); return Start.REFUSED }
        val (myGen, p) = started
        if (p == null) {
            val msg = spawnError?.message ?: "spawn failed"
            MeshState.log("✗ start failed: $msg"); MeshState.set { s -> s.copy(lastError = msg) }
            return Start.FAILED
        }
        MeshState.set { it.copy(processRunning = true, lastError = null) }
        MeshState.log("▶ ${cmd.drop(1).joinToString(" ")}")
        Thread {
            p.inputStream.bufferedReader().useLines { seq -> seq.forEach { MeshState.log(it.take(200)) } }
            val code = runCatching { p.waitFor() }.getOrDefault(-1)
            MeshState.log("■ $newRole process exited ($code)")
            if (gate.isCurrent(myGen)) { MeshState.set { it.copy(processRunning = false) }; onExit?.invoke(newRole, planId, code) }
        }.start()
        return Start.STARTED
    }

    /**
     * Anonymous RSS of our child process (what a stop returns to MemAvailable), for `Telemetry.held_bytes` (D024).
     * Android lets a parent read its own child's /proc status; 0 when nothing runs or the read fails.
     */
    fun heldBytes(): Long = proc?.let { p ->
        val pid = childPid(p)
        val v = if (pid > 0) runCatching { ProcStatus.rssAnonBytes(java.io.File("/proc/$pid/status").readText()) }.getOrDefault(0L) else 0L
        if (v == 0L && !warnedHeld) { warnedHeld = true; MeshState.log("⚠ cannot read the child's RSS (pid=$pid): meshd will credit nothing for this phone") }
        v
    } ?: 0L
    private var warnedHeld = false

    /**
     * The child's pid. `java.lang.Process.pid()` is not in the Android SDK for minSdk 30, so: the private `pid` field of
     * UNIXProcess (hidden-API policy permitting), else the public `toString()` which is "Process[pid=N, hasExited=…]".
     * NOT executed on a device yet (no arm64 phone on USB); a failure yields 0 → no credit (safe, D024/K19).
     */
    internal fun childPid(p: Process): Int =
        runCatching { p.javaClass.getDeclaredField("pid").apply { isAccessible = true }.getInt(p) }.getOrNull()
            ?: ProcStatus.pidFromToString(p.toString())

    /** Stop and disarm. Returns the (role, planId) that was running, so the caller can report it. */
    fun stop(): Pair<String, String>? {
        val had = gate.disarm { killProcess() } // under the gate lock; any exit callback of the old process is now stale
        MeshState.set { it.copy(processRunning = false) }
        return had
    }

    private fun killProcess() {
        proc?.let { p -> runCatching { p.destroy(); if (!p.waitFor(2, TimeUnit.SECONDS)) p.destroyForcibly() } }
        proc = null
    }

    /**
     * Fetch a model from the coordinator catalog if not cached. Resumable (M7). Returns the local file.
     * [expectedSha] is `Plan.model_sha` (proto/mesh.proto); when the plan carries one, a fully-present
     * file is trusted only if it hashes the same (T098) — otherwise it is verified by size only against
     * the coordinator's copy (a cheap out-of-range `Range` probe, same 416 path used to resume a `.part`
     * below), and that is said out loud rather than silently assumed.
     */
    suspend fun ensureModel(coordinatorApi: String, file: String, expectedSha: String = ""): File = withContext(Dispatchers.IO) {
        require(file.endsWith(".gguf") && !file.contains('/') && !file.contains("..")) { "bad model name" }
        val dest = File(modelsDir, file)
        if (dest.exists() && dest.length() > 0) {
            val by = ModelVerify.by(expectedSha)
            var reuse = false
            when (by) {
                ModelVerify.By.SHA256 -> {
                    val actual = runCatching { sha256Hex(dest) }.getOrNull()
                    when {
                        actual == null -> { MeshState.log(ModelVerify.unverifiedNote(dest.length())); reuse = true }
                        actual.equals(expectedSha, ignoreCase = true) -> { MeshState.log(ModelVerify.reusedNote(dest.length(), by)); reuse = true }
                        else -> { MeshState.log("✗ " + ModelVerify.mismatchNote(by)); dest.delete() }
                    }
                }
                ModelVerify.By.SIZE -> {
                    val remote = runCatching { remoteSize(coordinatorApi, file) }.getOrNull()
                    when {
                        remote == null -> { MeshState.log(ModelVerify.unverifiedNote(dest.length())); reuse = true }
                        remote == dest.length() -> { MeshState.log(ModelVerify.reusedNote(dest.length(), by)); reuse = true }
                        else -> { MeshState.log("✗ " + ModelVerify.mismatchNote(by)); dest.delete() }
                    }
                }
            }
            if (reuse) return@withContext dest
        }
        val part = File(modelsDir, "$file.part")
        val existing = if (part.exists()) part.length() else 0L
        val req = Request.Builder().url("$coordinatorApi/api/models/file/$file").apply { if (existing > 0) header("Range", "bytes=$existing-") }.build()
        http.newCall(req).execute().use { resp ->
            if (resp.code == 416) {
                // `Content-Range: bytes */len`: if our .part is already the whole file (app died before the rename), keep it.
                val len = resp.header("Content-Range")?.substringAfter("*/", "")?.toLongOrNull()
                if (len != null && len > 0 && part.length() == len) {
                    if (!part.renameTo(dest)) error("could not move ${part.name} into place")
                    MeshState.log("model cached: $file (already complete)")
                    return@withContext dest
                }
                part.delete(); error("resume rejected (416); partial file removed — retry")
            }
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
                        kotlinx.coroutines.currentCoroutineContext().ensureActive() // a real CancellationException: nothing is reported for a cancelled job (round-4 #1)
                        val n = inp.read(buf); if (n < 0) break
                        out.write(buf, 0, n); got += n
                        val pct = if (total > 0) (100 * got / total).toInt() else -1
                        if (pct != lastPct) { lastPct = pct; MeshState.set { it.copy(downloadPct = pct) }; onDownload?.invoke(pct) }
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

    private fun sha256Hex(file: File): String {
        val md = java.security.MessageDigest.getInstance("SHA-256")
        file.inputStream().use { inp ->
            val buf = ByteArray(1 shl 20)
            while (true) { val n = inp.read(buf); if (n < 0) break; md.update(buf, 0, n) }
        }
        return md.digest().joinToString("") { "%02x".format(it) }
    }

    /**
     * Total remote size without downloading anything: a `Range` past any real file's end makes meshd's
     * `api_model_file` answer 416 with a `Content-Range: bytes <wildcard>/<len>` header (same path the
     * resumed download above already parses) instead of streaming the body.
     */
    private fun remoteSize(coordinatorApi: String, file: String): Long? {
        val req = Request.Builder().url("$coordinatorApi/api/models/file/$file").header("Range", "bytes=999999999999999-").build()
        return http.newCall(req).execute().use { resp ->
            if (resp.code != 416) null else resp.header("Content-Range")?.substringAfter("*/", "")?.toLongOrNull()
        }
    }

    companion object {
        /**
         * Same rule as desktop/meshd/src/supervisor.rs `derive_args` (tested on the JVM in HostArgsTest; D018 keeps
         * the two in sync): llama.cpp counts the output head as a layer, so offload ngl+1, pin the head (and, for
         * tied-embedding models, its token_embd duplicate) to CPU with one regex, split the (ngl+1) offloaded
         * entries by worker layer counts with the extra head slot on the last worker, and pass `--fit off` so
         * llama.cpp never re-fits the placement the planner calculated (D033).
         */
        /** Pins the output head, its norm and (tied models) the token_embd duplicate to the host; same text as supervisor.rs. */
        const val HEAD_PIN = "^(output|output_norm|token_embd)\\.(weight|bias)$=CPU"

        fun hostArgs(serverBin: String, modelPath: String, nCtx: Int, threads: Int, bindHost: String, workers: List<Pair<String, Int>>, workerLayers: List<Int>): List<String> {
            val ngl = workerLayers.sum()
            val args = mutableListOf(serverBin, "-m", modelPath, "-c", "$nCtx", "-t", "$threads", "--host", bindHost, "--port", "8081", "--jinja", "--metrics", "--reasoning", "off", "--fit", "off")
            if (workers.isNotEmpty()) {
                args += listOf("--rpc", workers.joinToString(",") { "${it.first}:${it.second}" }, "-ngl", "${ngl + 1}", "--override-tensor", HEAD_PIN)
                if (workers.size > 1) {
                    val total = (ngl + 1).toDouble()
                    val split = workerLayers.mapIndexed { i, n -> (if (i == workerLayers.lastIndex) n + 1 else n) / total }
                    args += listOf("--tensor-split", split.joinToString(",") { String.format(java.util.Locale.ROOT, "%.6f", it) })
                }
            } else args += listOf("-ngl", "0")
            return args
        }
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
