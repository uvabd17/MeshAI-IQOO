package ai.meshai.worker

import ai.meshai.worker.core.LlamaRunner
import ai.meshai.worker.core.MeshService
import ai.meshai.worker.core.MeshState
import ai.meshai.worker.core.PairingPayload
import ai.meshai.worker.core.Profiler
import ai.meshai.worker.ui.Dashboard
import ai.meshai.worker.ui.MeshTheme
import android.Manifest
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.lifecycle.lifecycleScope
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private val scan = registerForActivityResult(ScanContract()) { r -> r.contents?.let { join(it) } }
    private val perms = registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {}

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Stay top-app (all cores in the cpuset) and keep the Wi-Fi low-latency lock effective while participating.
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        val wanted = mutableListOf(Manifest.permission.CAMERA)
        if (Build.VERSION.SDK_INT >= 33) wanted += "android.permission.POST_NOTIFICATIONS"
        perms.launch(wanted.toTypedArray())
        offerFromIntent(intent)
        setContent {
            MeshTheme {
                val s by MeshState.ui.collectAsState()
                Dashboard(
                    s = s,
                    onScan = { scan.launch(ScanOptions().setDesiredBarcodeFormats(ScanOptions.QR_CODE).setBeepEnabled(false).setOrientationLocked(true).setPrompt("Scan the MeshAI pairing code")) },
                    onJoin = ::join,
                    onConfirmJoin = { p -> MeshState.set { it.copy(pendingJoin = null) }; join(p.toJson()) },
                    onRejectJoin = { MeshState.set { it.copy(pendingJoin = null) } },
                    onLeave = { MeshService.leave(this) },
                    onStop = { MeshService.stopProcess(this) },
                    onBench = ::bench,
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        offerFromIntent(intent)
    }

    /**
     * A payload handed to us by another app or adb (`am start … --es payload '<json>'`) is shown as a
     * request the user must confirm on screen (M9) — never auto-joined. QR scans in-app join directly.
     */
    private fun offerFromIntent(intent: Intent?) {
        val p = intent?.getStringExtra(EXTRA_PAYLOAD)?.let { PairingPayload.parse(it) } ?: return
        MeshState.set { it.copy(pendingJoin = p) }
        MeshState.log("pairing request from another app for ${p.meshId} @ ${p.host} — waiting for confirmation")
    }

    private fun join(payload: String) {
        if (!payload.contains("token")) { Toast.makeText(this, "Not a MeshAI pairing code", Toast.LENGTH_SHORT).show(); return }
        MeshService.join(this, payload)
    }

    private fun bench() {
        val runner = LlamaRunner(this)
        val model = runner.modelsDir.listFiles()?.filter { it.name.endsWith(".gguf") }?.minByOrNull { it.length() }
        if (model == null) { Toast.makeText(this, "No model cached on the phone yet", Toast.LENGTH_SHORT).show(); return }
        val threads = Profiler(this).workerThreads()
        MeshState.log("bench ${model.name} with $threads threads…")
        lifecycleScope.launch(Dispatchers.IO) {
            val tps = runner.bench(model, threads)
            MeshState.set { it.copy(decodeTps = tps) }
            MeshState.log("bench: %.1f tok/s".format(tps))
        }
    }

    companion object { const val EXTRA_PAYLOAD = "payload" }
}
