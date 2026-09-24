package ai.meshai.worker.ui

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

@Composable private fun NBox(modifier: Modifier = Modifier, fill: Color? = null, shadow: Dp = 6.dp, radius: Dp = 12.dp, pad: Dp = 16.dp, content: @Composable () -> Unit) {
    val t = LocalNb.current
    Box(modifier.padding(end = shadow, bottom = shadow)) {
        Box(Modifier.matchParentSize().offset(shadow, shadow).background(t.ink, RoundedCornerShape(radius)))
        Box(Modifier.fillMaxWidth().background(fill ?: t.paper, RoundedCornerShape(radius)).border(BW, t.ink, RoundedCornerShape(radius)).padding(pad)) { content() }
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
@Composable private fun BigIcon(icon: ImageVector, fill: Color? = null) {
    val t = LocalNb.current
    Box(Modifier.size(52.dp).background(fill ?: t.accent, RoundedCornerShape(10.dp)).border(BW, t.ink, RoundedCornerShape(10.dp)), contentAlignment = Alignment.Center) {
        Icon(icon, null, Modifier.size(32.dp), tint = Color(0xFF0A0A0A).takeIf { (fill ?: t.accent) == t.accent } ?: t.ink)
    }
}
/** Card head: big icon + plain title + one-line meaning. */
@Composable private fun Head(icon: ImageVector, title: String, meaning: String, fill: Color? = null) {
    val t = LocalNb.current
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        BigIcon(icon, fill)
        Column(Modifier.weight(1f)) {
            Text(title, fontWeight = FontWeight.Black, fontSize = 18.sp, color = t.ink)
            Muted(meaning, 12)
        }
    }
}

/* ================= screen with two tabs ================= */

@Composable
fun Dashboard(s: UiState, onScan: () -> Unit, onJoin: (String) -> Unit, onConfirmJoin: (PairingPayload) -> Unit, onRejectJoin: () -> Unit, onLeave: () -> Unit, onStop: () -> Unit, onBench: () -> Unit) {
    val t = LocalNb.current
    var tab by rememberSaveable { mutableStateOf(0) }
    Column(Modifier.fillMaxSize().background(t.paper)) {
        LazyColumn(Modifier.weight(1f).padding(horizontal = 18.dp), contentPadding = PaddingValues(top = 12.dp, bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(18.dp)) {
            item { Spacer(Modifier.statusBarsPadding().height(26.dp)) }
            item { Header(s) }
            s.pendingJoin?.let { p -> item { PendingJoinCard(p, onConfirmJoin, onRejectJoin) } }
            if (tab == 0) {
                if (!s.paired) item { JoinCard(onScan, onJoin, s.lastError) }
                else {
                    item { StatusHero(s, onStop, onLeave) }
                    if (s.mesh.isNotEmpty()) item { MeshCard(s) } else item { WaitingCard() }
                }
                item { ActionsRow(s, onBench, onScan, onLeave) }
                item { LogCard(s) }
            } else {
                item { SpeedCard(s, onBench) }
                item { CoresCard(s) }
                item { MemoryCard(s) }
                item { HeatCard(s) }
                item { BatteryCard(s) }
                item { ConnectionCard(s) }
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

@Composable private fun StatusHero(s: UiState, onStop: () -> Unit, onLeave: () -> Unit) {
    val t = LocalNb.current
    val fill = when (s.role) { "host" -> t.host; "worker" -> t.worker; else -> t.paper2 }
    NBox(fill = fill, shadow = 8.dp) {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) { Label("This phone is"); Spacer(Modifier.weight(1f)); Sticker(if (s.processRunning) "working" else "idle", fill = if (s.processRunning) t.accent else t.paper) }
            Text(when (s.role) { "host" -> "THE BRAIN"; "worker" -> "A HELPER"; else -> "READY" }, fontWeight = FontWeight.Black, fontSize = 34.sp, letterSpacing = (-1).sp, color = t.ink, lineHeight = 36.sp)
            Text(when {
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
            s.mesh.sortedBy { if (it.role == "host") 0 else if (it.role == "worker") 1 else 2 }.forEach { p -> PeerRow(p, s.nLayer) }
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
    NBox(fill = fill, shadow = 3.dp, radius = 10.dp, pad = 12.dp) {
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Box(Modifier.size(48.dp).background(t.paper, RoundedCornerShape(8.dp)).border(BW, t.ink, RoundedCornerShape(8.dp)), contentAlignment = Alignment.Center) {
                    Icon(if (p.kind == "laptop") Icons.Outlined.Laptop else Icons.Outlined.PhoneAndroid, null, Modifier.size(30.dp), tint = t.ink)
                }
                Column(Modifier.weight(1f)) {
                    Text(p.name, fontWeight = FontWeight.Black, fontSize = 15.sp, color = t.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(when (p.role) { "host" -> "THE BRAIN · RUNS THE MODEL"; "worker" -> "HELPER · COMPUTES LAYERS"; else -> "NOT NEEDED" } + if (p.isMe) "  ·  ME" else "", fontSize = 10.sp, letterSpacing = 1.5.sp, fontWeight = FontWeight.Black, color = t.ink)
                }
            }
            if (p.spec.isNotEmpty()) Mono(p.spec, 11, t.ink)
            if (used) {
                val n = p.layerEnd - p.layerStart
                Text("holds layers ${p.layerStart}–${maxOf(p.layerStart, p.layerEnd - 1)}  ·  $n of $nLayer  ·  ${gb(p.bytes)} of memory", fontSize = 12.sp, fontWeight = FontWeight.Black, color = t.ink)
                LayerBar(p.layerStart, p.layerEnd, nLayer)
            } else if (p.reason.isNotEmpty()) Muted(p.reason.substringAfter(": ").take(100), 11)
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
    NBox(fill = t.accent, shadow = 8.dp) {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Head(Icons.Outlined.Speed, "Speed", "How many words per second this phone can write.", fill = t.paper)
            Row(verticalAlignment = Alignment.CenterVertically) {
                Gauge(value = s.decodeTps, max = 40f, Modifier.size(150.dp))
                Spacer(Modifier.width(16.dp))
                Column {
                    Text(if (s.decodeTps > 0) "%.1f".format(s.decodeTps) else "—", fontSize = 44.sp, fontWeight = FontWeight.Black, color = Color(0xFF0A0A0A), fontFamily = FontFamily.Monospace, letterSpacing = (-2).sp)
                    Text("tokens per second", fontSize = 13.sp, fontWeight = FontWeight.Bold, color = Color(0xFF0A0A0A))
                    Text(when { s.decodeTps <= 0 -> "run a speed test"; s.decodeTps < 5 -> "slow · fine for short answers"; s.decodeTps < 15 -> "good · reads like typing"; else -> "fast · faster than you read" }, fontSize = 12.sp, color = Color(0xFF0A0A0A))
                }
            }
            NButton("Run speed test", Modifier.fillMaxWidth(), fill = t.paper, icon = Icons.Outlined.Bolt) { onBench() }
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
    NBox {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Head(Icons.Outlined.Memory, "Cores at work", "Each bar is one CPU core. Taller = running faster right now.")
            if (s.coreLoads.isEmpty()) Muted("waiting for the first sample…")
            else Row(Modifier.fillMaxWidth().height(120.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.Bottom) {
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
            Rule()
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                Legend(t.accent, "prime"); Legend(t.worker, "big"); Legend(t.host, "small")
                Spacer(Modifier.weight(1f))
                Mono("${s.threads} threads for the model", 11, t.ink)
            }
        }
    }
}

@Composable private fun Legend(c: Color, text: String) { val t = LocalNb.current; Row(verticalAlignment = Alignment.CenterVertically) { Box(Modifier.size(12.dp).background(c).border(2.dp, t.ink)); Spacer(Modifier.width(5.dp)); Mono(text, 10, t.ink) } }

@Composable private fun MemoryCard(s: UiState) {
    val t = LocalNb.current
    val total = s.totalBytes.coerceAtLeast(1)
    val free = s.availBytes.coerceIn(0, total)
    val usable = (free - 1_500_000_000L).coerceAtLeast(0)
    NBox {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Head(Icons.Outlined.Memory, "Memory", "How much room there is for a model.")
            Row(verticalAlignment = Alignment.Bottom) {
                Text(if (s.availBytes > 0) gb(usable) else "—", fontSize = 40.sp, fontWeight = FontWeight.Black, color = t.ink, fontFamily = FontFamily.Monospace, letterSpacing = (-2).sp)
                Spacer(Modifier.width(8.dp)); Text("usable for models", fontSize = 13.sp, fontWeight = FontWeight.Bold, color = t.ink, modifier = Modifier.padding(bottom = 8.dp))
            }
            Row(Modifier.fillMaxWidth().height(28.dp).background(t.paper, RoundedCornerShape(6.dp)).border(BW, t.ink, RoundedCornerShape(6.dp)).padding(3.dp), horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                val usedF = ((total - free).toFloat() / total).coerceIn(0.02f, 1f)
                val keepF = ((free - usable).toFloat() / total).coerceIn(0.02f, 1f)
                val usableF = (usable.toFloat() / total).coerceAtLeast(0.02f)
                Box(Modifier.weight(usedF).fillMaxHeight().background(t.paper3(), RoundedCornerShape(3.dp)).border(2.dp, t.ink, RoundedCornerShape(3.dp)))
                Box(Modifier.weight(keepF).fillMaxHeight().background(t.danger, RoundedCornerShape(3.dp)).border(2.dp, t.ink, RoundedCornerShape(3.dp)))
                Box(Modifier.weight(usableF).fillMaxHeight().background(t.host, RoundedCornerShape(3.dp)).border(2.dp, t.ink, RoundedCornerShape(3.dp)))
            }
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) { Legend(t.paper3(), "other apps ${gb(total - free)}"); Legend(t.danger, "kept safe 1.5 GB"); Legend(t.host, "for models") }
            Mono("${gb(free)} free of ${gb(total)} total", 11)
        }
    }
}
private fun NbTokens.paper3() = paper2

@Composable private fun HeatCard(s: UiState) {
    val t = LocalNb.current
    val h = s.thermalHeadroom // 0 = cool … 1 = throttling
    val word = when { h <= 0f -> "Cool"; h < 0.5f -> "Warm"; h < 0.85f -> "Hot"; else -> "Too hot" }
    val fill = when { h < 0.5f -> t.host; h < 0.85f -> t.accent; else -> t.danger }
    NBox {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Head(Icons.Outlined.Thermostat, "Heat", "Phones slow down when they get hot.", fill = fill)
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Thermometer(h.coerceIn(0f, 1f), fill, Modifier.width(44.dp).height(130.dp))
                Column {
                    Text(word.uppercase(), fontSize = 34.sp, fontWeight = FontWeight.Black, color = t.ink, letterSpacing = (-1).sp)
                    Text(when { h <= 0f -> "Nothing to worry about."; h < 0.5f -> "Fine for long runs."; h < 0.85f -> "It will start slowing down soon."; else -> "Let it cool before running a model." }, fontSize = 13.sp, color = t.ink, fontWeight = FontWeight.Medium)
                    Mono("headroom %.2f · status %d".format(h, s.thermalStatus), 11)
                }
            }
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
    NBox {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Head(if (s.charging) Icons.Outlined.BatteryChargingFull else Icons.Outlined.BatteryFull, "Battery", if (s.charging) "Plugged in — best for long runs." else "Running a model drains the battery faster.")
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                Row(Modifier.weight(1f).height(40.dp), verticalAlignment = Alignment.CenterVertically) {
                    Row(Modifier.weight(1f).fillMaxHeight().background(t.paper, RoundedCornerShape(6.dp)).border(BW, t.ink, RoundedCornerShape(6.dp)).padding(4.dp)) {
                        val f = (s.batteryPct / 100f).coerceIn(0.02f, 1f)
                        Box(Modifier.weight(f).fillMaxHeight().background(if (s.batteryPct < 20) t.danger else t.host, RoundedCornerShape(3.dp)).border(2.dp, t.ink, RoundedCornerShape(3.dp)))
                        if (f < 1f) Spacer(Modifier.weight(1f - f))
                    }
                    Box(Modifier.width(8.dp).height(18.dp).background(t.ink, RoundedCornerShape(topEnd = 3.dp, bottomEnd = 3.dp)))
                }
                Text(if (s.batteryPct > 0) "${s.batteryPct}%" else "—", fontSize = 36.sp, fontWeight = FontWeight.Black, color = t.ink, fontFamily = FontFamily.Monospace, letterSpacing = (-2).sp)
            }
            Mono(if (s.charging) "charging" else "on battery", 11)
        }
    }
}

@Composable private fun ConnectionCard(s: UiState) {
    val t = LocalNb.current
    val rtt = s.rttP50
    val word = when { rtt <= 0f -> "—"; rtt < 5f -> "Excellent"; rtt < 15f -> "Good"; rtt < 40f -> "OK"; else -> "Slow" }
    NBox {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Head(Icons.Outlined.Wifi, "Connection to the laptop", "A fast link lets the two devices share work.", fill = when { rtt <= 0f -> t.paper2; rtt < 15f -> t.host; rtt < 40f -> t.accent; else -> t.danger })
            Row(verticalAlignment = Alignment.Bottom) {
                Text(word.uppercase(), fontSize = 30.sp, fontWeight = FontWeight.Black, color = t.ink, letterSpacing = (-1).sp)
                Spacer(Modifier.weight(1f))
                Mono(if (rtt > 0) "%.1f ms round trip".format(rtt) else "not measured yet", 12, t.ink)
            }
            Sparkline(s.rttHistory, Modifier.fillMaxWidth().height(70.dp))
            Rule()
            Row { Mono("laptop", 11); Spacer(Modifier.weight(1f)); Mono(s.coordinator.ifEmpty { "—" }, 12, t.ink) }
            Row { Mono("link", 11); Spacer(Modifier.weight(1f)); Mono(if (s.connected) "connected · checked every 10 s" else "reconnecting…", 12, t.ink) }
            Row { Mono("worst recently", 11); Spacer(Modifier.weight(1f)); Mono(if (s.rttP95 > 0) "%.1f ms".format(s.rttP95) else "—", 12, t.ink) }
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
    NBox(shadow = 4.dp) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Head(Icons.Outlined.PhoneAndroid, "This phone", "What the laptop knows about it.")
            Row { Mono("chip", 11); Spacer(Modifier.weight(1f)); Mono(s.socName.ifEmpty { "—" }, 12, t.ink) }
            Row { Mono("system", 11); Spacer(Modifier.weight(1f)); Mono(s.osName.ifEmpty { "—" }, 12, t.ink) }
            Row { Mono("cores the app may use", 11); Spacer(Modifier.weight(1f)); Mono(s.cpusAllowed, 12, t.ink) }
            Row { Mono("capability", 11); Spacer(Modifier.weight(1f)); Mono(if (s.tier.isEmpty()) "—" else "tier ${s.tier} · arm64 + dotprod + i8mm", 12, t.ink) }
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
