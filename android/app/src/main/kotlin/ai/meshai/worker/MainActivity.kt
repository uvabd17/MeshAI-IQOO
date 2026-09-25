package ai.meshai.worker

import ai.meshai.worker.core.AutoJoin
import ai.meshai.worker.core.LlamaRunner
import ai.meshai.worker.core.MeshService
import ai.meshai.worker.core.MeshState
import ai.meshai.worker.core.PairingPayload
import ai.meshai.worker.core.Profiler
import ai.meshai.worker.ui.Dashboard
import ai.meshai.worker.ui.MeshTheme
import android.Manifest
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
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
        refreshKeepAlive()
        // Already paired earlier? Reconnect by ourselves — no scan, no tap (the secret is ours). Guarded so
        // this never races a payload the user still has to confirm, and never starts a second job to a mesh
        // we're already live on (a superseded ControlClient job used to hold a silent link until meshd
        // dropped it 45 s later, taking the healthy connection's llama.cpp process down with it).
        val savedPayload = getSharedPreferences("meshai", Context.MODE_PRIVATE).getString("last_payload", null)
        val ui = MeshState.ui.value
        val autoJoin = AutoJoin.shouldAutoJoin(
            hasIntentPayload = intent?.getStringExtra(EXTRA_PAYLOAD) != null,
            pendingJoinShown = ui.pendingJoin != null,
            liveMeshId = ui.meshId.takeIf { ui.connected },
            savedMeshId = savedPayload?.let { PairingPayload.parse(it)?.meshId },
        )
        if (autoJoin) { savedPayload?.let { MeshService.join(this, it) } }
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
                    onRequestNoBatteryRestriction = ::requestNoBatteryRestriction,
                    onOpenAppSettings = ::openAppSettings,
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        offerFromIntent(intent)
    }

    /** These are only ever changed from Settings, outside the app, so a fresh read on every resume
     *  (e.g. coming back from the Settings screen the two buttons below open) is enough (T098). */
    override fun onResume() {
        super.onResume()
        refreshKeepAlive()
    }

    private fun refreshKeepAlive() {
        val k = Profiler(this).keepAlive()
        MeshState.set { it.copy(batteryUnrestricted = k.batteryUnrestricted, stayOnWhilePluggedIn = k.stayOnWhilePluggedIn, screenTimeoutMin = k.screenTimeoutMin) }
    }

    /** ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS is allowed without a runtime prompt for a foreground-service
     *  app holding REQUEST_IGNORE_BATTERY_OPTIMIZATIONS (manifest); the system still shows its own confirm dialog. */
    private fun requestNoBatteryRestriction() {
        val i = Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:$packageName"))
        runCatching { startActivity(i) }.onFailure { Toast.makeText(this, "Could not open the battery settings", Toast.LENGTH_SHORT).show() }
    }

    /** Autostart (MIUI/HyperOS) has no public read or write API — send the user to the app's own info page. */
    private fun openAppSettings() {
        val i = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:$packageName"))
        runCatching { startActivity(i) }.onFailure { Toast.makeText(this, "Could not open app settings", Toast.LENGTH_SHORT).show() }
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
