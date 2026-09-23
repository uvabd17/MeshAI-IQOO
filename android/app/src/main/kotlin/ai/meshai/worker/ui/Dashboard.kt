package ai.meshai.worker.ui

import ai.meshai.worker.core.UiState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Bolt
import androidx.compose.material.icons.outlined.Memory
import androidx.compose.material.icons.outlined.QrCodeScanner
import androidx.compose.material.icons.outlined.Thermostat
import androidx.compose.material.icons.outlined.Wifi
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

private fun gb(b: Long) = "%.1f GB".format(b / 1e9)

@Composable
fun Dashboard(s: UiState, onScan: () -> Unit, onJoin: (String) -> Unit, onConfirmJoin: (ai.meshai.worker.core.PairingPayload) -> Unit, onRejectJoin: () -> Unit, onLeave: () -> Unit, onStop: () -> Unit, onBench: () -> Unit) {
    val c = MaterialTheme.colorScheme
    LazyColumn(Modifier.background(c.background).padding(horizontal = 18.dp), contentPadding = androidx.compose.foundation.layout.PaddingValues(top = 22.dp, bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        item { Header(s) }
        s.pendingJoin?.let { p -> item { PendingJoinCard(p, onConfirmJoin, onRejectJoin) } }
        if (!s.paired) item { JoinCard(onScan, onJoin, s.lastError) }
        else item { RoleCard(s, onLeave, onStop) }
        item { MetricsGrid(s) }
        item { Card { Column { Row(verticalAlignment = Alignment.CenterVertically) { Label("PROCESS LOG"); Spacer(Modifier.weight(1f)); OutlinedButton(onClick = onBench, contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 4.dp)) { Text("Bench", fontSize = 12.sp) } }; Spacer(Modifier.height(6.dp)) } } }
        items(s.log.takeLast(40)) { Text(it, fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = c.onSurfaceVariant, lineHeight = 15.sp) }
    }
}

@Composable private fun Header(s: UiState) {
    val c = MaterialTheme.colorScheme
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(38.dp).background(c.primary, RoundedCornerShape(9.dp)), contentAlignment = Alignment.Center) { Text("M", color = c.onPrimary, fontWeight = FontWeight.Black, fontSize = 18.sp) }
        Spacer(Modifier.width(12.dp))
        Column { Text("MeshAI", fontWeight = FontWeight.Bold, fontSize = 18.sp, color = c.onBackground); Text(if (s.meshId.isEmpty()) "not paired" else s.meshId, fontSize = 11.sp, fontFamily = FontFamily.Monospace, color = c.onSurfaceVariant) }
        Spacer(Modifier.weight(1f))
        Pill(text = when { !s.paired -> "idle"; !s.connected -> "reconnecting"; else -> s.role }, on = s.connected)
    }
}

@Composable private fun Pill(text: String, on: Boolean) {
    val c = MaterialTheme.colorScheme
    Row(Modifier.border(1.dp, c.outline, CircleShape).padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(8.dp).background(if (on) c.primary else c.onSurfaceVariant, CircleShape)); Spacer(Modifier.width(8.dp)); Text(text, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, color = c.onBackground)
    }
}

@Composable private fun Card(content: @Composable () -> Unit) {
    val c = MaterialTheme.colorScheme
    Box(Modifier.fillMaxWidth().background(c.surface, RoundedCornerShape(14.dp)).border(1.dp, c.outline, RoundedCornerShape(14.dp)).padding(16.dp)) { content() }
}
@Composable private fun Label(t: String) = Text(t, fontSize = 11.sp, letterSpacing = 1.sp, fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.onSurfaceVariant)

@Composable private fun JoinCard(onScan: () -> Unit, onJoin: (String) -> Unit, err: String?) {
    var payload by remember { mutableStateOf("") }
    val c = MaterialTheme.colorScheme
    Card {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Label("JOIN A MESH")
            Text("On the laptop open MeshAI → Devices → New QR, then scan it here. Both devices must be on your own hotspot or USB tethering.", fontSize = 13.sp, color = c.onSurfaceVariant)
            Button(onClick = onScan, modifier = Modifier.fillMaxWidth(), colors = ButtonDefaults.buttonColors(containerColor = c.primary, contentColor = c.onPrimary)) { Icon(Icons.Outlined.QrCodeScanner, null); Spacer(Modifier.width(8.dp)); Text("Scan QR code") }
            OutlinedTextField(value = payload, onValueChange = { payload = it }, modifier = Modifier.fillMaxWidth(), label = { Text("or paste the pairing payload") }, maxLines = 3)
            OutlinedButton(onClick = { onJoin(payload) }, modifier = Modifier.fillMaxWidth(), enabled = payload.contains("token")) { Text("Join") }
            if (err != null) Text("✗ $err", fontSize = 12.sp, color = c.onBackground)
        }
    }
}

@Composable private fun PendingJoinCard(p: ai.meshai.worker.core.PairingPayload, onConfirm: (ai.meshai.worker.core.PairingPayload) -> Unit, onReject: () -> Unit) {
    val c = MaterialTheme.colorScheme
    Card {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Label("PAIRING REQUEST")
            Text("Another app asked this phone to join ${p.meshId} at ${p.host}:${p.controlPort}. Only accept if that is your own laptop.", fontSize = 13.sp, color = c.onSurfaceVariant)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = { onConfirm(p) }, colors = ButtonDefaults.buttonColors(containerColor = c.primary, contentColor = c.onPrimary)) { Text("Join") }
                OutlinedButton(onClick = onReject) { Text("Ignore") }
            }
        }
    }
}

@Composable private fun RoleCard(s: UiState, onLeave: () -> Unit, onStop: () -> Unit) {
    val c = MaterialTheme.colorScheme
    Card {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Label("THIS PHONE")
            Text(when (s.role) { "host" -> "Host — running the model, serving answers"; "worker" -> "Compute worker"; else -> "Paired, waiting for a plan" }, fontWeight = FontWeight.SemiBold, fontSize = 15.sp, color = c.onBackground)
            if (s.layerEnd > s.layerStart) {
                Text("layers ${s.layerStart}–${s.layerEnd - 1}  ·  ${s.layerEnd - s.layerStart} of the model  ·  ${s.threads} threads", fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = c.onSurfaceVariant)
                LayerBar(s.layerStart, s.layerEnd)
            }
            if (s.planSummary.isNotEmpty()) Text(s.planSummary, fontSize = 12.sp, color = c.onSurfaceVariant)
            if (s.modelFile.isNotEmpty()) Text(s.modelFile, fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = c.onSurfaceVariant)
            if (s.downloadPct >= 0) { Text("fetching model… ${s.downloadPct}%", fontSize = 12.sp, color = c.onSurfaceVariant); LinearProgressIndicator(progress = { s.downloadPct / 100f }, modifier = Modifier.fillMaxWidth(), color = c.primary, trackColor = c.surfaceVariant) }
            Text("coordinator ${s.coordinator}  ·  llama.cpp ${if (s.processRunning) "running" else "stopped"}", fontSize = 11.sp, fontFamily = FontFamily.Monospace, color = c.onSurfaceVariant)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = onStop, enabled = s.processRunning) { Text("Stop process") }
                OutlinedButton(onClick = onLeave) { Text("Leave mesh") }
            }
        }
    }
}

@Composable private fun LayerBar(start: Int, end: Int) {
    val c = MaterialTheme.colorScheme
    val total = maxOf(end, 48)
    Row(Modifier.fillMaxWidth().height(10.dp).background(c.surfaceVariant, RoundedCornerShape(5.dp))) {
        if (start > 0) Spacer(Modifier.weight(start.toFloat()))
        Box(Modifier.weight((end - start).toFloat().coerceAtLeast(0.1f)).height(10.dp).background(c.primary, RoundedCornerShape(5.dp)))
        if (total - end > 0) Spacer(Modifier.weight((total - end).toFloat()))
    }
}

@Composable private fun MetricsGrid(s: UiState) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Metric(Modifier.weight(1f), Icons.Outlined.Memory, "FREE RAM", if (s.availBytes > 0) gb(s.availBytes) else "—", if (s.totalBytes > 0) "of ${gb(s.totalBytes)} · usable ${gb((s.availBytes - 1_500_000_000L).coerceAtLeast(0))}" else "")
            Metric(Modifier.weight(1f), Icons.Outlined.Thermostat, "THERMAL", if (s.thermalHeadroom > 0) "%.2f".format(s.thermalHeadroom) else "—", "headroom · status ${s.thermalStatus}")
        }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Metric(Modifier.weight(1f), Icons.Outlined.Bolt, "BATTERY", if (s.batteryPct > 0) "${s.batteryPct}%" else "—", if (s.charging) "charging" else "on battery")
            Metric(Modifier.weight(1f), Icons.Outlined.Wifi, "LINK RTT", if (s.rttP50 > 0) "%.1f ms".format(s.rttP50) else "—", if (s.rttP95 > 0) "p95 %.1f ms".format(s.rttP95) else "")
        }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Metric(Modifier.weight(1f), Icons.Outlined.Memory, "CPUSET", s.cpusAllowed, "cores this app may use")
            Metric(Modifier.weight(1f), Icons.Outlined.Bolt, "DECODE", if (s.decodeTps > 0) "%.1f tok/s".format(s.decodeTps) else "—", "last bench")
        }
    }
}

@Composable private fun Metric(m: Modifier, icon: ImageVector, k: String, v: String, sub: String) {
    val c = MaterialTheme.colorScheme
    Column(m.background(c.surface, RoundedCornerShape(12.dp)).border(1.dp, c.outline, RoundedCornerShape(12.dp)).padding(12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) { Icon(icon, null, Modifier.size(14.dp), tint = c.onSurfaceVariant); Spacer(Modifier.width(6.dp)); Label(k) }
        Text(v, fontSize = 20.sp, fontWeight = FontWeight.Bold, color = c.onBackground, fontFamily = FontFamily.Monospace)
        if (sub.isNotEmpty()) Text(sub, fontSize = 10.sp, color = c.onSurfaceVariant)
    }
}
