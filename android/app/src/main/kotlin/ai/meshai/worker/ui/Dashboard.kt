package ai.meshai.worker.ui

import ai.meshai.worker.core.CacheSummary
import ai.meshai.worker.core.KeepAliveRows
import ai.meshai.worker.core.MeshPeer
import ai.meshai.worker.core.PairingPayload
import ai.meshai.worker.core.UiState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.BatteryChargingFull
import androidx.compose.material.icons.outlined.BatteryFull
import androidx.compose.material.icons.outlined.Bolt
import androidx.compose.material.icons.outlined.Hub
import androidx.compose.material.icons.outlined.Laptop
import androidx.compose.material.icons.outlined.Memory
import androidx.compose.material.icons.outlined.PhoneAndroid
import androidx.compose.material.icons.outlined.QrCodeScanner
import androidx.compose.material.icons.outlined.Speed
import androidx.compose.material.icons.outlined.Thermostat
import androidx.compose.material.icons.outlined.Wifi
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

private fun gb(b: Long) = "%.1f GB".format(b / 1e9)
private val BW = 3.dp

/* ================= neobrutalist primitives ================= */

@Composable private fun NBox(modifier: Modifier = Modifier, fill: Color? = null, shadow: Dp = 6.dp, radius: Dp = 12.dp, pad: Dp = 16.dp, fillHeight: Boolean = false, content: @Composable () -> Unit) {
    val t = LocalNb.current
    Box(modifier.padding(end = shadow, bottom = shadow)) {
        Box(Modifier.matchParentSize().offset(shadow, shadow).background(t.ink, RoundedCornerShape(radius)))
        Box(Modifier.fillMaxWidth().then(if (fillHeight) Modifier.fillMaxHeight() else Modifier).background(fill ?: t.paper, RoundedCornerShape(radius)).border(BW, t.ink, RoundedCornerShape(radius)).padding(pad)) { content() }
    }
}

/** Two cards side by side with equal height. */
@Composable private fun Grid2(a: @Composable () -> Unit, b: @Composable () -> Unit) {
    Row(Modifier.fillMaxWidth().height(androidx.compose.foundation.layout.IntrinsicSize.Max), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.weight(1f).fillMaxHeight()) { a() }
        Box(Modifier.weight(1f).fillMaxHeight()) { b() }
    }
}

@Composable private fun NButton(text: String, modifier: Modifier = Modifier, fill: Color? = null, enabled: Boolean = true, icon: ImageVector? = null, big: Boolean = false, onClick: () -> Unit) {
    val t = LocalNb.current
    val src = remember { MutableInteractionSource() }
    val pressed by src.collectIsPressedAsState()
    val shadow = if (pressed) 0.dp else 4.dp
    val bg = when { !enabled -> t.paper2; fill != null -> fill; else -> t.paper }
    val fg = if (fill == t.accent) Color(0xFF0A0A0A) else t.ink
    Box(modifier.padding(end = 4.dp, bottom = 4.dp)) {
        Box(Modifier.matchParentSize().offset(4.dp, 4.dp).background(if (enabled) t.ink else Color.Transparent, RoundedCornerShape(8.dp)))
        Row(
            Modifier.offset(4.dp - shadow, 4.dp - shadow).fillMaxWidth().background(bg, RoundedCornerShape(8.dp)).border(BW, if (enabled) t.ink else t.muted, RoundedCornerShape(8.dp))
                .clickable(interactionSource = src, indication = null, enabled = enabled, onClick = onClick).padding(horizontal = 14.dp, vertical = if (big) 16.dp else 12.dp),
            horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically,
        ) {
            if (icon != null) { Icon(icon, null, Modifier.size(if (big) 24.dp else 18.dp), tint = if (enabled) fg else t.muted); Spacer(Modifier.width(8.dp)) }
            Text(text.uppercase(), fontWeight = FontWeight.Black, fontSize = if (big) 15.sp else 13.sp, letterSpacing = 1.sp, color = if (enabled) fg else t.muted)
        }
    }
}

@Composable private fun Sticker(text: String, fill: Color? = null, rotate: Float = -3f) {
    val t = LocalNb.current
    Box(Modifier.rotate(rotate).padding(end = 2.dp, bottom = 2.dp)) {
        Box(Modifier.matchParentSize().offset(2.dp, 2.dp).background(t.ink, RoundedCornerShape(4.dp)))
        Text(text.uppercase(), Modifier.background(fill ?: t.accent, RoundedCornerShape(4.dp)).border(2.dp, t.ink, RoundedCornerShape(4.dp)).padding(horizontal = 9.dp, vertical = 3.dp),
            fontWeight = FontWeight.Black, fontSize = 10.sp, letterSpacing = 1.5.sp, color = if ((fill ?: t.accent) == t.accent) Color(0xFF0A0A0A) else t.ink)
    }
}

@Composable private fun Label(t: String) = Text(t.uppercase(), fontSize = 11.sp, letterSpacing = 2.sp, fontWeight = FontWeight.Black, color = LocalNb.current.ink)
@Composable private fun Muted(t: String, size: Int = 12) = Text(t, fontSize = size.sp, color = LocalNb.current.muted, fontWeight = FontWeight.Medium)
@Composable private fun Mono(t: String, size: Int = 11, color: Color? = null) = Text(t, fontFamily = FontFamily.Monospace, fontSize = size.sp, color = color ?: LocalNb.current.muted, fontWeight = FontWeight.SemiBold)
@Composable private fun Rule() { val t = LocalNb.current; Box(Modifier.fillMaxWidth().height(BW).background(t.ink)) }
@Composable private fun BigIcon(icon: ImageVector, fill: Color? = null, size: Dp = 52.dp) {
    val t = LocalNb.current
    Box(Modifier.size(size).background(fill ?: t.accent, RoundedCornerShape(10.dp)).border(BW, t.ink, RoundedCornerShape(10.dp)), contentAlignment = Alignment.Center) {
        Icon(icon, null, Modifier.size(size * 0.6f), tint = Color(0xFF0A0A0A).takeIf { (fill ?: t.accent) == t.accent } ?: t.ink)
    }
}
/** Card head: big icon + plain title + one-line meaning. `small` for grid cards. */
@Composable private fun Head(icon: ImageVector, title: String, meaning: String, fill: Color? = null, small: Boolean = false) {
    val t = LocalNb.current
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(if (small) 8.dp else 12.dp)) {
        BigIcon(icon, fill, if (small) 38.dp else 52.dp)
        Column(Modifier.weight(1f)) {
            Text(title, fontWeight = FontWeight.Black, fontSize = if (small) 15.sp else 18.sp, color = t.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (!small) Muted(meaning, 12) else Muted(meaning, 10)
        }
    }
}

/* ================= screen with two tabs ================= */

@Composable
fun Dashboard(
    s: UiState, onScan: () -> Unit, onJoin: (String) -> Unit, onConfirmJoin: (PairingPayload) -> Unit, onRejectJoin: () -> Unit,
    onLeave: () -> Unit, onStop: () -> Unit, onBench: () -> Unit,
    onRequestNoBatteryRestriction: () -> Unit = {}, onOpenAppSettings: () -> Unit = {},
) {
    val t = LocalNb.current
    var tab by rememberSaveable { mutableStateOf(0) }
    Column(Modifier.fillMaxSize().background(t.paper).statusBarsPadding()) {
        LazyColumn(Modifier.weight(1f).padding(horizontal = 18.dp), contentPadding = PaddingValues(top = 28.dp, bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(18.dp)) {
            item { Header(s) }
            if (s.paired || s.connected) item { SetupSteps(s) }
            s.pendingJoin?.let { p -> item { PendingJoinCard(p, onConfirmJoin, onRejectJoin) } }
            if (tab == 0) {
                if (!s.paired) item { JoinCard(onScan, onJoin, s.lastError) }
                else {
                    item { StatusHero(s, onStop, onLeave) }
                    if (s.mesh.isNotEmpty()) item { MeshCard(s) } else item { WaitingCard() }
                    CacheSummary.meshTabLine(s.rpcCacheBytes, s.rpcCacheFiles)?.let { line -> item { CacheCard(line) } }
                }
                item { ActionsRow(s, onBench, onScan, onLeave) }
                item { LogCard(s) }
            } else {
                item { Grid2({ SpeedCard(s, onBench) }, { CoresCard(s) }) }
                item { Grid2({ MemoryCard(s) }, { HeatCard(s) }) }
                item { Grid2({ BatteryCard(s) }, { ConnectionCard(s) }) }
                item { KeepAliveCard(s, onRequestNoBatteryRestriction, onOpenAppSettings) }
                item { AboutCard(s) }
            }
        }
        BottomTabs(tab) { tab = it }
    }
}

@Composable private fun BottomTabs(tab: Int, onTab: (Int) -> Unit) {
    val t = LocalNb.current
    Column(Modifier.fillMaxWidth().background(t.paper)) {
        Rule()
        Row(Modifier.navigationBarsPadding().padding(horizontal = 14.dp, vertical = 10.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            TabButton("Mesh", Icons.Outlined.Hub, tab == 0, Modifier.weight(1f)) { onTab(0) }
            TabButton("This phone", Icons.Outlined.PhoneAndroid, tab == 1, Modifier.weight(1f)) { onTab(1) }
        }
    }
}

@Composable private fun TabButton(text: String, icon: ImageVector, on: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val t = LocalNb.current
    Box(modifier.padding(end = 4.dp, bottom = 4.dp)) {
        if (!on) Box(Modifier.matchParentSize().offset(4.dp, 4.dp).background(t.ink, RoundedCornerShape(10.dp)))
        Row(
            Modifier.offset(if (on) 4.dp else 0.dp, if (on) 4.dp else 0.dp).fillMaxWidth().background(if (on) t.ink else t.paper, RoundedCornerShape(10.dp)).border(BW, t.ink, RoundedCornerShape(10.dp))
                .clickable(onClick = onClick).padding(vertical = 14.dp),
            horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(icon, null, Modifier.size(24.dp), tint = if (on) t.paper else t.ink); Spacer(Modifier.width(10.dp))
            Text(text.uppercase(), fontWeight = FontWeight.Black, fontSize = 14.sp, letterSpacing = 1.sp, color = if (on) t.paper else t.ink)
        }
    }
}

@Composable private fun Header(s: UiState) {
    val t = LocalNb.current
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.padding(end = 4.dp, bottom = 4.dp)) {
            Box(Modifier.size(52.dp).offset(4.dp, 4.dp).background(t.ink, RoundedCornerShape(12.dp)))
            Box(Modifier.size(52.dp).background(t.accent, RoundedCornerShape(12.dp)).border(BW, t.ink, RoundedCornerShape(12.dp)), contentAlignment = Alignment.Center) { Text("M", color = Color(0xFF0A0A0A), fontWeight = FontWeight.Black, fontSize = 26.sp) }
        }
        Spacer(Modifier.width(14.dp))
        Column {
            Text("MESHAI", fontWeight = FontWeight.Black, fontSize = 24.sp, letterSpacing = (-0.5).sp, color = t.ink)
            Mono(if (s.meshId.isEmpty()) "not paired yet" else s.meshId, 11)
        }
        Spacer(Modifier.weight(1f))
        Sticker(when { !s.paired -> "idle"; !s.connected -> "reconnecting"; s.role == "idle" -> "paired"; else -> s.role }, fill = when { !s.paired -> t.paper2; !s.connected -> t.danger; s.role == "host" -> t.host; s.role == "worker" -> t.worker; else -> t.accent })
    }
}

/* ================= setup stepper ================= */

private data class Step(val title: String, val detail: String, val done: Boolean, val active: Boolean, val progress: Float? = null)

private fun steps(s: UiState): List<Step> {
    val planned = s.mesh.isNotEmpty() && s.role != "idle"
    val host = s.role == "host"
    val downloading = s.downloadPct in 0..99
    val modelReady = host && (s.processRunning || s.downloadPct == 100)
    val engine = s.processRunning
    val ready = if (host) s.processRunning else s.workerReady
    val l = mutableListOf<Step>()
    l += Step("Connect to the laptop", if (s.connected) "linked to ${s.coordinator}" else "dialling ${s.coordinator.ifEmpty { "…" }}", s.connected, !s.connected)
    l += Step("Pair", if (s.paired) "secret stored for ${s.meshId}" else "waiting for the laptop to accept", s.paired, s.connected && !s.paired)
    l += Step("Scan this phone", if (s.profileSent) "chip, cores, memory and heat sent" else "reading chip, cores, memory…", s.profileSent, s.paired && !s.profileSent)
    l += Step("Get a plan", if (planned) "I am the ${if (host) "brain" else "helper"}" else "the laptop decides who holds what", planned, s.profileSent && !planned)
    if (host) l += Step("Get the model", when { downloading -> "${s.downloadPct}% from the laptop"; modelReady -> "on this phone"; else -> "from the laptop" }, modelReady, planned && !modelReady, if (downloading) s.downloadPct / 100f else null)
    l += Step("Start the engine", if (engine) "llama.cpp is running" else "launching llama.cpp", engine, planned && (host && modelReady || !host) && !engine)
    l += Step("Ready", if (ready) (if (host) "answering the laptop's questions" else "computing my layers for the laptop") else "almost there", ready, engine && !ready)
    return l
}

@Composable private fun SetupSteps(s: UiState) {
    val t = LocalNb.current
    val st = steps(s)
    val done = st.count { it.done }
    val allDone = st.all { it.done }
    NBox(fill = if (allDone) t.host else t.paper, shadow = 5.dp, pad = 12.dp) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Label(if (allDone) "All set" else "Setting up"); Spacer(Modifier.weight(1f))
                if (!allDone) Sticker("$done / ${st.size}", fill = t.accent)
                else Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) { st.forEach { _ -> Box(Modifier.size(14.dp).background(t.ink, RoundedCornerShape(3.dp))) } }
            }
            if (!allDone) {
                Bar(done.toFloat() / st.size)
                st.chunked(2).forEach { pair ->
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        pair.forEach { step -> StepChip(step, Modifier.weight(1f)) }
                        if (pair.size == 1) Spacer(Modifier.weight(1f))
                    }
                }
                st.firstOrNull { it.active }?.let { a -> Muted("Now: ${a.title} — ${a.detail}", 11); a.progress?.let { Bar(it) } }
            } else Muted(st.last().detail, 11)
        }
    }
}

@Composable private fun StepChip(step: Step, modifier: Modifier) {
    val t = LocalNb.current
    Row(modifier.background(if (step.active) t.accent else t.paper, RoundedCornerShape(6.dp)).border(2.dp, if (step.done || step.active) t.ink else t.muted, RoundedCornerShape(6.dp)).padding(horizontal = 8.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Box(Modifier.size(18.dp).background(if (step.done) t.ink else t.paper, RoundedCornerShape(4.dp)).border(2.dp, t.ink, RoundedCornerShape(4.dp)), contentAlignment = Alignment.Center) {
            when { step.done -> Text("✓", color = t.paper, fontWeight = FontWeight.Black, fontSize = 11.sp); step.active -> CircularProgressIndicator(Modifier.size(10.dp), color = t.ink, strokeWidth = 2.dp); else -> {} }
        }
        Text(step.title, fontWeight = FontWeight.Black, fontSize = 11.sp, color = if (step.done || step.active) t.ink else t.muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/* ================= MESH tab ================= */

@Composable private fun PendingJoinCard(p: PairingPayload, onConfirm: (PairingPayload) -> Unit, onReject: () -> Unit) {
    val t = LocalNb.current
    NBox(fill = t.accent) {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text("A LAPTOP WANTS TO PAIR", fontWeight = FontWeight.Black, fontSize = 13.sp, letterSpacing = 2.sp, color = Color(0xFF0A0A0A))
            Text("${p.meshId} at ${p.host}:${p.controlPort}. Only accept if that is your own laptop.", fontSize = 14.sp, color = Color(0xFF0A0A0A), fontWeight = FontWeight.SemiBold)
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                NButton("Join", Modifier.weight(1f), fill = t.paper) { onConfirm(p) }
                NButton("Ignore", Modifier.weight(1f), fill = t.paper) { onReject() }
            }
        }
    }
}

@Composable private fun JoinCard(onScan: () -> Unit, onJoin: (String) -> Unit, err: String?) {
    var payload by remember { mutableStateOf("") }
    val t = LocalNb.current
    NBox {
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Head(Icons.Outlined.QrCodeScanner, "Join a mesh", "Scan the QR shown on the laptop's MeshAI screen.")
            Text("Both devices must be on your own hotspot, Wi-Fi or USB tethering.", fontSize = 13.sp, color = t.ink, fontWeight = FontWeight.Medium)
            NButton("Scan QR code", Modifier.fillMaxWidth(), fill = t.accent, icon = Icons.Outlined.QrCodeScanner, big = true) { onScan() }
            OutlinedTextField(value = payload, onValueChange = { payload = it }, modifier = Modifier.fillMaxWidth(), label = { Text("or paste the pairing text") }, maxLines = 3)
            NButton("Join with text", Modifier.fillMaxWidth(), enabled = payload.contains("token")) { onJoin(payload) }
            if (err != null) Text("✗ $err", fontSize = 12.sp, color = t.ink, fontWeight = FontWeight.Bold)
        }
    }
}

@Composable private fun WaitingCard() {
    val t = LocalNb.current
    NBox(fill = t.paper2, shadow = 4.dp) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Head(Icons.Outlined.Hub, "No plan yet", "On the laptop, pick a model and press Run.", fill = t.paper)
            Text("The laptop decides who holds which part of the model. You will see it here the moment it does.", fontSize = 13.sp, color = t.ink, fontWeight = FontWeight.Medium)
        }
    }
}

/** "Layers cached on this phone: 234 MB (12 files)" — what a resumed worker plan will reuse (T098). */
@Composable private fun CacheCard(line: String) {
    val t = LocalNb.current
    NBox(fill = t.paper2, shadow = 3.dp, pad = 12.dp) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Outlined.Memory, null, Modifier.size(22.dp), tint = t.ink)
            Spacer(Modifier.width(10.dp))
            Column {
                Text(line, fontWeight = FontWeight.Black, fontSize = 13.sp, color = t.ink)
                Muted("From an earlier run. Whether a re-run reuses them has not been measured yet.", 11)
            }
        }
    }
}

@Composable private fun StatusHero(s: UiState, onStop: () -> Unit, onLeave: () -> Unit) {
    val t = LocalNb.current
    val fill = when (s.role) { "host" -> t.host; "worker" -> t.worker; else -> t.paper2 }
    NBox(fill = fill, shadow = 8.dp) {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) { Label("This phone is"); Spacer(Modifier.weight(1f)); Sticker(if (s.processRunning) "working" else "idle", fill = if (s.processRunning) t.accent else t.paper) }
            Text(when { !s.connected -> "RECONNECTING"; s.role == "host" -> "THE BRAIN"; s.role == "worker" -> "A HELPER"; else -> "READY" }, fontWeight = FontWeight.Black, fontSize = 34.sp, letterSpacing = (-1).sp, color = t.ink, lineHeight = 36.sp)
            Text(when {
                // A dropped link during a run (H2/round-4 #3): say what stays, but only claim the cache when
                // there actually is one (an empty claim would be a lie, T098) — and never claim what a
                // resumed run does with it, since that has not been measured (D035).
                !s.connected && s.rpcCacheBytes > 0 -> "Lost the laptop — reconnecting. Your cached layers stay on the phone."
                !s.connected -> "Lost the laptop — reconnecting."
                s.downloadPct in 0..99 -> "Getting the model from the laptop · ${s.downloadPct}%"
                s.role == "host" && s.processRunning -> "This phone runs the whole model and writes the answers. The laptop just passes your questions here."
                s.role == "worker" && s.processRunning -> "This phone holds ${s.layerEnd - s.layerStart} of the model's ${if (s.nLayer > 0) s.nLayer else "?"} layers and computes them for the laptop."
                s.role == "host" -> "Starting the model on this phone…"
                s.role == "worker" -> "Starting this phone's compute engine…"
                else -> "Waiting for the laptop to send a plan."
            }, fontSize = 14.sp, color = t.ink, fontWeight = FontWeight.SemiBold)
            if (s.downloadPct in 0..99) Bar(s.downloadPct / 100f)
            if (s.layerEnd > s.layerStart) {
                Rule()
                Row { Mono("my layers ${s.layerStart}–${s.layerEnd - 1}", 12, t.ink); Spacer(Modifier.weight(1f)); Mono("${s.threads} threads", 12, t.ink) }
                LayerBar(s.layerStart, s.layerEnd, s.nLayer)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                NButton("Stop", Modifier.weight(1f), enabled = s.processRunning || s.downloadPct in 0..99) { onStop() }
                NButton("Leave mesh", Modifier.weight(1f), fill = t.paper) { onLeave() }
            }
        }
    }
}

@Composable private fun MeshCard(s: UiState) {
    val t = LocalNb.current
    NBox {
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Head(Icons.Outlined.Hub, "Who holds what", "The model is cut into layers and shared out.")
            Box(Modifier.fillMaxWidth().background(t.paper2, RoundedCornerShape(8.dp)).border(BW, t.ink, RoundedCornerShape(8.dp)).padding(12.dp)) {
                Column {
                    Text(s.modelLabel, fontWeight = FontWeight.Black, fontSize = 17.sp, color = t.ink)
                    Mono("${s.nLayer} layers · remembers ${s.nCtx} tokens · ${s.mesh.count { it.role != "unused" }} device(s) working", 12)
                }
            }
            StackBar(s)
            s.mesh.sortedBy { if (it.role == "host") 0 else if (it.role == "worker") 1 else 2 }.chunked(2).forEach { pair ->
                Row(Modifier.fillMaxWidth().height(androidx.compose.foundation.layout.IntrinsicSize.Max), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    pair.forEach { p -> Box(Modifier.weight(1f).fillMaxHeight()) { PeerRow(p, s.nLayer) } }
                    if (pair.size == 1) Spacer(Modifier.weight(1f))
                }
            }
        }
    }
}

/** One bar for the whole model: each device's slice, host first. */
@Composable private fun StackBar(s: UiState) {
    val t = LocalNb.current
    val used = s.mesh.filter { it.role != "unused" && it.layerEnd > it.layerStart }.sortedBy { it.layerStart }
    if (used.isEmpty() || s.nLayer <= 0) return
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(Modifier.fillMaxWidth().height(26.dp).background(t.paper, RoundedCornerShape(6.dp)).border(BW, t.ink, RoundedCornerShape(6.dp)).padding(3.dp), horizontalArrangement = Arrangement.spacedBy(3.dp)) {
            used.forEach { p ->
                Box(Modifier.weight((p.layerEnd - p.layerStart).toFloat()).fillMaxHeight().background(if (p.role == "host") t.host else t.worker, RoundedCornerShape(3.dp)).border(2.dp, t.ink, RoundedCornerShape(3.dp)), contentAlignment = Alignment.Center) {
                    Text("${p.layerEnd - p.layerStart}", fontSize = 11.sp, fontWeight = FontWeight.Black, color = t.ink)
                }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            used.forEach { p -> Row(verticalAlignment = Alignment.CenterVertically) { Box(Modifier.size(10.dp).background(if (p.role == "host") t.host else t.worker).border(1.dp, t.ink)); Spacer(Modifier.width(4.dp)); Mono(p.name.take(18) + if (p.isMe) " (me)" else "", 10) } }
        }
    }
}

@Composable private fun PeerRow(p: MeshPeer, nLayer: Int) {
    val t = LocalNb.current
    val used = p.role != "unused"
    val fill = when (p.role) { "host" -> t.host; "worker" -> t.worker; else -> t.paper2 }
    NBox(fill = fill, shadow = 3.dp, radius = 10.dp, pad = 10.dp, fillHeight = true) {
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Box(Modifier.size(40.dp).background(t.paper, RoundedCornerShape(8.dp)).border(BW, t.ink, RoundedCornerShape(8.dp)), contentAlignment = Alignment.Center) {
                Icon(if (p.kind == "laptop") Icons.Outlined.Laptop else Icons.Outlined.PhoneAndroid, null, Modifier.size(26.dp), tint = t.ink)
            }
            Text(p.name, fontWeight = FontWeight.Black, fontSize = 13.sp, color = t.ink, maxLines = 2, overflow = TextOverflow.Ellipsis, lineHeight = 15.sp)
            Text(when (p.role) { "host" -> "THE BRAIN"; "worker" -> "HELPER"; else -> "NOT NEEDED" } + if (p.isMe) " · ME" else "", fontSize = 10.sp, letterSpacing = 1.5.sp, fontWeight = FontWeight.Black, color = t.ink)
            if (p.spec.isNotEmpty()) Mono(p.spec.split(" · ").take(3).joinToString(" · "), 10, t.ink)
            if (used) {
                val n = p.layerEnd - p.layerStart
                Text("layers ${p.layerStart}–${maxOf(p.layerStart, p.layerEnd - 1)}\n$n of $nLayer · ${gb(p.bytes)}", fontSize = 11.sp, fontWeight = FontWeight.Black, color = t.ink, lineHeight = 14.sp)
                LayerBar(p.layerStart, p.layerEnd, nLayer)
            } else if (p.reason.isNotEmpty()) Muted(p.reason.substringAfter(": ").take(60), 10)
        }
    }
}

@Composable private fun ActionsRow(s: UiState, onBench: () -> Unit, onScan: () -> Unit, onLeave: () -> Unit) {
    val t = LocalNb.current
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Label("Actions")
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            NButton("Speed test", Modifier.weight(1f), fill = t.accent, icon = Icons.Outlined.Speed) { onBench() }
            NButton(if (s.paired) "Re-pair" else "Scan", Modifier.weight(1f), icon = Icons.Outlined.QrCodeScanner) { onScan() }
        }
    }
}

@Composable private fun LogCard(s: UiState) {
    val t = LocalNb.current
    NBox(shadow = 4.dp, pad = 0.dp) {
        Column {
            Row(Modifier.padding(horizontal = 14.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) { Label("What happened"); Spacer(Modifier.weight(1f)); Mono("${s.log.size} events", 10) }
            Rule()
            Column(Modifier.fillMaxWidth().background(t.ink, RoundedCornerShape(bottomStart = 10.dp, bottomEnd = 10.dp)).padding(12.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                if (s.log.isEmpty()) Text("nothing yet", fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = t.paper)
                s.log.takeLast(14).forEach { Text(it, fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = t.paper, lineHeight = 15.sp) }
            }
        }
    }
}

/* ================= THIS PHONE tab ================= */

@Composable private fun SpeedCard(s: UiState, onBench: () -> Unit) {
    val t = LocalNb.current
    NBox(fill = t.accent, shadow = 5.dp, pad = 12.dp, fillHeight = true) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Head(Icons.Outlined.Speed, "Speed", "words per second", fill = t.paper, small = true)
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                Gauge(value = s.decodeTps, max = 40f, Modifier.size(110.dp))
                Text(if (s.decodeTps > 0) "%.1f".format(s.decodeTps) else "—", fontSize = 26.sp, fontWeight = FontWeight.Black, color = Color(0xFF0A0A0A), fontFamily = FontFamily.Monospace, letterSpacing = (-1).sp)
            }
            Text(when { s.decodeTps <= 0 -> "run a speed test"; s.decodeTps < 5 -> "slow · short answers"; s.decodeTps < 15 -> "good · like typing"; else -> "fast" }, fontSize = 11.sp, fontWeight = FontWeight.Bold, color = Color(0xFF0A0A0A))
            Spacer(Modifier.weight(1f))
            NButton("Test", Modifier.fillMaxWidth(), fill = t.paper, icon = Icons.Outlined.Bolt) { onBench() }
        }
    }
}

@Composable private fun Gauge(value: Float, max: Float, modifier: Modifier) {
    val t = LocalNb.current
    val frac = (value / max).coerceIn(0f, 1f)
    Canvas(modifier) {
        val stroke = 22f
        val r = Size(size.width - stroke, size.height - stroke)
        val tl = Offset(stroke / 2, stroke / 2)
        drawArc(t.ink, 135f, 270f, false, tl, r, style = Stroke(stroke + 8f, cap = StrokeCap.Butt))
        drawArc(t.paper, 135f, 270f, false, tl, r, style = Stroke(stroke, cap = StrokeCap.Butt))
        if (frac > 0f) drawArc(t.ink, 135f, 270f * frac, false, tl, r, style = Stroke(stroke, cap = StrokeCap.Butt))
    }
}

@Composable private fun CoresCard(s: UiState) {
    val t = LocalNb.current
    NBox(pad = 12.dp, fillHeight = true) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Head(Icons.Outlined.Memory, "Cores", "taller = busier", small = true)
            if (s.coreLoads.isEmpty()) Muted("waiting…")
            else Row(Modifier.fillMaxWidth().height(96.dp), horizontalArrangement = Arrangement.spacedBy(4.dp), verticalAlignment = Alignment.Bottom) {
                s.coreLoads.forEachIndexed { i, l ->
                    val cap = s.coreCaps.getOrNull(i) ?: 1024
                    val fill = when { cap >= 1000 -> t.accent; cap >= 500 -> t.worker; else -> t.host }
                    Column(Modifier.weight(1f).fillMaxHeight(), verticalArrangement = Arrangement.Bottom, horizontalAlignment = Alignment.CenterHorizontally) {
                        Box(Modifier.fillMaxWidth().weight((1f - l).coerceAtLeast(0.02f)))
                        Box(Modifier.fillMaxWidth().weight(l.coerceAtLeast(0.06f)).background(fill, RoundedCornerShape(topStart = 4.dp, topEnd = 4.dp)).border(2.dp, t.ink, RoundedCornerShape(topStart = 4.dp, topEnd = 4.dp)))
                        Mono("$i", 10, t.ink)
                    }
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Legend(t.accent, "prime"); Legend(t.worker, "big"); Legend(t.host, "small")
            }
            Mono("${s.threads} threads for the model", 10, t.ink)
        }
    }
}

@Composable private fun Legend(c: Color, text: String) { val t = LocalNb.current; Row(verticalAlignment = Alignment.CenterVertically) { Box(Modifier.size(12.dp).background(c).border(2.dp, t.ink)); Spacer(Modifier.width(5.dp)); Mono(text, 10, t.ink) } }

@Composable private fun MemoryCard(s: UiState) {
    val t = LocalNb.current
    val total = s.totalBytes.coerceAtLeast(1)
    val free = s.availBytes.coerceIn(0, total)
    val usable = (free - 1_500_000_000L).coerceAtLeast(0)
    NBox(pad = 12.dp, fillHeight = true) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Head(Icons.Outlined.Memory, "Memory", "room for a model", small = true)
            Text(if (s.availBytes > 0) gb(usable) else "—", fontSize = 28.sp, fontWeight = FontWeight.Black, color = t.ink, fontFamily = FontFamily.Monospace, letterSpacing = (-1).sp)
            Muted("usable for models", 10)
            Row(Modifier.fillMaxWidth().height(28.dp).background(t.paper, RoundedCornerShape(6.dp)).border(BW, t.ink, RoundedCornerShape(6.dp)).padding(3.dp), horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                val usedF = ((total - free).toFloat() / total).coerceIn(0.02f, 1f)
                val keepF = ((free - usable).toFloat() / total).coerceIn(0.02f, 1f)
                val usableF = (usable.toFloat() / total).coerceAtLeast(0.02f)
                Box(Modifier.weight(usedF).fillMaxHeight().background(t.paper3(), RoundedCornerShape(3.dp)).border(2.dp, t.ink, RoundedCornerShape(3.dp)))
                Box(Modifier.weight(keepF).fillMaxHeight().background(t.danger, RoundedCornerShape(3.dp)).border(2.dp, t.ink, RoundedCornerShape(3.dp)))
                Box(Modifier.weight(usableF).fillMaxHeight().background(t.host, RoundedCornerShape(3.dp)).border(2.dp, t.ink, RoundedCornerShape(3.dp)))
            }
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) { Legend(t.paper3(), "apps ${gb(total - free)}"); Legend(t.danger, "safety 1.5 GB"); Legend(t.host, "models ${gb(usable)}") }
            Mono("${gb(free)} free / ${gb(total)}", 10)
        }
    }
}
private fun NbTokens.paper3() = paper2

@Composable private fun HeatCard(s: UiState) {
    val t = LocalNb.current
    val h = s.thermalHeadroom // 0 = cool … 1 = throttling
    val word = when { h <= 0f -> "Cool"; h < 0.5f -> "Warm"; h < 0.85f -> "Hot"; else -> "Too hot" }
    val fill = when { h < 0.5f -> t.host; h < 0.85f -> t.accent; else -> t.danger }
    NBox(pad = 12.dp, fillHeight = true) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Head(Icons.Outlined.Thermostat, "Heat", "hot = slower", fill = fill, small = true)
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Thermometer(h.coerceIn(0f, 1f), fill, Modifier.width(34.dp).height(100.dp))
                Column {
                    Text(word.uppercase(), fontSize = 22.sp, fontWeight = FontWeight.Black, color = t.ink, letterSpacing = (-1).sp)
                    Text(when { h <= 0f -> "all good"; h < 0.5f -> "fine for long runs"; h < 0.85f -> "slowing soon"; else -> "let it cool" }, fontSize = 11.sp, color = t.ink, fontWeight = FontWeight.Medium)
                }
            }
            Mono("headroom %.2f · status %d".format(h, s.thermalStatus), 10)
        }
    }
}

@Composable private fun Thermometer(frac: Float, fill: Color, modifier: Modifier) {
    val t = LocalNb.current
    Canvas(modifier) {
        val w = size.width; val hgt = size.height
        val bulbR = w / 2f
        val tubeW = w * 0.45f
        val tubeL = w / 2f - tubeW / 2f
        val tubeTop = tubeW / 2f
        val tubeBottom = hgt - bulbR * 1.6f
        val p = Path().apply { addRoundRect(androidx.compose.ui.geometry.RoundRect(tubeL, tubeTop, tubeL + tubeW, tubeBottom + bulbR / 2f, androidx.compose.ui.geometry.CornerRadius(tubeW / 2f))) }
        drawPath(p, t.paper); drawPath(p, t.ink, style = Stroke(6f))
        drawCircle(fill, bulbR, Offset(w / 2f, hgt - bulbR)); drawCircle(t.ink, bulbR, Offset(w / 2f, hgt - bulbR), style = Stroke(6f))
        val level = tubeBottom - (tubeBottom - tubeTop - 8f) * frac
        drawRoundRect(fill, Offset(tubeL + 6f, level), Size(tubeW - 12f, tubeBottom + bulbR / 2f - level), androidx.compose.ui.geometry.CornerRadius(6f))
        for (i in 0..4) { val y = tubeTop + (tubeBottom - tubeTop) * i / 4f; drawLine(t.ink, Offset(tubeL + tubeW + 6f, y), Offset(tubeL + tubeW + 16f, y), 4f) }
    }
}

@Composable private fun BatteryCard(s: UiState) {
    val t = LocalNb.current
    NBox(pad = 12.dp, fillHeight = true) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Head(if (s.charging) Icons.Outlined.BatteryChargingFull else Icons.Outlined.BatteryFull, "Battery", if (s.charging) "plugged in" else "on battery", small = true)
            Text(if (s.batteryPct > 0) "${s.batteryPct}%" else "—", fontSize = 28.sp, fontWeight = FontWeight.Black, color = t.ink, fontFamily = FontFamily.Monospace, letterSpacing = (-1).sp)
            Row(Modifier.fillMaxWidth().height(30.dp), verticalAlignment = Alignment.CenterVertically) {
                Row(Modifier.weight(1f).fillMaxHeight().background(t.paper, RoundedCornerShape(6.dp)).border(BW, t.ink, RoundedCornerShape(6.dp)).padding(3.dp)) {
                    val f = (s.batteryPct / 100f).coerceIn(0.02f, 1f)
                    Box(Modifier.weight(f).fillMaxHeight().background(if (s.batteryPct < 20) t.danger else t.host, RoundedCornerShape(3.dp)).border(2.dp, t.ink, RoundedCornerShape(3.dp)))
                    if (f < 1f) Spacer(Modifier.weight(1f - f))
                }
                Box(Modifier.width(6.dp).height(14.dp).background(t.ink, RoundedCornerShape(topEnd = 3.dp, bottomEnd = 3.dp)))
            }
            Muted(if (s.charging) "best for long runs" else "a model drains it faster", 10)
        }
    }
}

/** What stops this phone being killed mid-run (T098): plain rows plus the two toggles we can actually open for the user. */
@Composable private fun KeepAliveCard(s: UiState, onRequestNoBatteryRestriction: () -> Unit, onOpenAppSettings: () -> Unit) {
    val t = LocalNb.current
    val rows = KeepAliveRows.rows(s.batteryUnrestricted, s.stayOnWhilePluggedIn, s.screenTimeoutMin)
    NBox(shadow = 4.dp, pad = 12.dp) {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Head(Icons.Outlined.Bolt, "Keep it running", "what can stop this phone mid-run", small = true)
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                rows.forEach { r ->
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                        Muted(r.label, 12); Mono(r.value, 12, t.ink)
                    }
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                NButton("Request no battery restriction", Modifier.weight(1f), fill = t.accent) { onRequestNoBatteryRestriction() }
                NButton("Open app settings", Modifier.weight(1f)) { onOpenAppSettings() }
            }
        }
    }
}

@Composable private fun ConnectionCard(s: UiState) {
    val t = LocalNb.current
    val rtt = s.rttP50
    val word = when { rtt <= 0f -> "—"; rtt < 5f -> "Excellent"; rtt < 15f -> "Good"; rtt < 40f -> "OK"; else -> "Slow" }
    NBox(pad = 12.dp, fillHeight = true) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Head(Icons.Outlined.Wifi, "Link", "to the laptop", fill = when { rtt <= 0f -> t.paper2; rtt < 15f -> t.host; rtt < 40f -> t.accent; else -> t.danger }, small = true)
            Text(word.uppercase(), fontSize = 22.sp, fontWeight = FontWeight.Black, color = t.ink, letterSpacing = (-1).sp)
            Mono(if (rtt > 0) "%.1f ms · worst %.0f".format(rtt, s.rttP95) else "not measured yet", 10, t.ink)
            Sparkline(s.rttHistory, Modifier.fillMaxWidth().height(48.dp))
            Mono(if (s.connected) "connected" else "reconnecting…", 10, t.ink)
        }
    }
}

@Composable private fun Sparkline(values: List<Float>, modifier: Modifier) {
    val t = LocalNb.current
    Box(modifier.background(t.paper, RoundedCornerShape(6.dp)).border(BW, t.ink, RoundedCornerShape(6.dp)).padding(6.dp)) {
        Canvas(Modifier.fillMaxSize()) {
            if (values.size < 2) return@Canvas
            val max = (values.maxOrNull() ?: 1f).coerceAtLeast(1f)
            val step = size.width / (values.size - 1)
            val path = Path()
            values.forEachIndexed { i, v -> val x = i * step; val y = size.height - (v / max) * size.height * 0.9f - 2f; if (i == 0) path.moveTo(x, y) else path.lineTo(x, y) }
            drawPath(path, t.ink, style = Stroke(5f, cap = StrokeCap.Round))
            val last = values.last(); drawCircle(t.accent, 9f, Offset(size.width, size.height - (last / max) * size.height * 0.9f - 2f)); drawCircle(t.ink, 9f, Offset(size.width, size.height - (last / max) * size.height * 0.9f - 2f), style = Stroke(4f))
        }
        Mono("%.0f ms".format(values.maxOrNull() ?: 0f), 9, t.muted)
    }
}

@Composable private fun AboutCard(s: UiState) {
    val t = LocalNb.current
    NBox(shadow = 4.dp, pad = 12.dp) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Head(Icons.Outlined.PhoneAndroid, "This phone", "what the laptop knows about it", small = true)
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Mono("chip", 10); Text(s.socName.ifEmpty { "—" }, fontSize = 12.sp, fontWeight = FontWeight.Black, color = t.ink)
                    Mono("system", 10); Text(s.osName.ifEmpty { "—" }, fontSize = 12.sp, fontWeight = FontWeight.Black, color = t.ink)
                }
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Mono("cores the app may use", 10); Text(s.cpusAllowed, fontSize = 12.sp, fontWeight = FontWeight.Black, color = t.ink)
                    Mono("laptop", 10); Text(s.coordinator.ifEmpty { "—" }, fontSize = 12.sp, fontWeight = FontWeight.Black, color = t.ink)
                }
            }
            Mono(if (s.tier.isEmpty()) "—" else "tier ${s.tier} · arm64 + dotprod + i8mm", 10)
        }
    }
}

/* ================= small visuals ================= */

@Composable private fun LayerBar(start: Int, end: Int, nLayer: Int = 0) {
    val t = LocalNb.current
    val total = if (nLayer > 0) nLayer else maxOf(end, 48)
    Row(Modifier.fillMaxWidth().height(18.dp).background(t.paper, RoundedCornerShape(4.dp)).border(2.dp, t.ink, RoundedCornerShape(4.dp)).padding(2.dp)) {
        if (start > 0) Spacer(Modifier.weight(start.toFloat()))
        Box(Modifier.weight((end - start).toFloat().coerceAtLeast(0.1f)).height(14.dp).background(t.ink, RoundedCornerShape(2.dp)))
        if (total - end > 0) Spacer(Modifier.weight((total - end).toFloat()))
    }
}

@Composable private fun Bar(frac: Float) {
    val t = LocalNb.current
    Row(Modifier.fillMaxWidth().height(16.dp).background(t.paper, RoundedCornerShape(4.dp)).border(2.dp, t.ink, RoundedCornerShape(4.dp)).padding(2.dp)) {
        Box(Modifier.weight(frac.coerceIn(0.01f, 1f)).height(12.dp).background(t.ink, RoundedCornerShape(2.dp)))
        if (frac < 1f) Spacer(Modifier.weight((1f - frac).coerceAtLeast(0.01f)))
    }
}
