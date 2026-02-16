package com.zerodevi1.chaos_seed.player.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.offset
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.material3.Text
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shadow
import com.zerodevi1.chaos_seed.core.model.DanmakuMessage
import kotlin.math.roundToInt

/**
 * A lightweight "bilibili-like" danmaku: messages fly from right to left in multiple lanes.
 *
 * This is intentionally simple (no collision-avoidance beyond basic lane spacing) so we can keep
 * CPU use reasonable and avoid pulling in heavy third-party libs.
 */
@Composable
fun DanmakuFlyingOverlay(
    messages: List<DanmakuMessage>,
    enabled: Boolean,
    modifier: Modifier = Modifier,
) {
    val density = LocalDensity.current

    // Active bullets currently animating.
    val active = remember { mutableStateListOf<Bullet>() }
    // Prevent duplicates when backend emits small bursts.
    val seen = remember { mutableStateMapOf<String, Boolean>() }

    // Lane pacing (ms) to reduce overlaps.
    val laneLastStart = remember { mutableStateMapOf<Int, Long>() }
    var laneCursor by remember { mutableIntStateOf(0) }

    if (!enabled) {
        LaunchedEffect(Unit) {
            active.clear()
            seen.clear()
            laneLastStart.clear()
        }
        return
    }

    BoxWithConstraints(modifier = modifier) {
        val widthPx = with(density) { maxWidth.toPx() }
        val heightPx = with(density) { maxHeight.toPx() }

        val fontSize = 16.sp
        val lineHeightPx = with(density) { 22.sp.toPx() }
        val laneCount = (heightPx / lineHeightPx).toInt().coerceIn(3, 10)
        val laneGapMs = 520L

        fun pickLane(now: Long): Int {
            for (i in 0 until laneCount) {
                val lane = (laneCursor + i) % laneCount
                val last = laneLastStart[lane] ?: 0L
                if (now - last >= laneGapMs) {
                    laneCursor = (lane + 1) % laneCount
                    laneLastStart[lane] = now
                    return lane
                }
            }
            // All lanes are busy; just round-robin.
            val lane = laneCursor % laneCount
            laneCursor = (laneCursor + 1) % laneCount
            laneLastStart[lane] = now
            return lane
        }

        // Convert new messages -> bullets.
        LaunchedEffect(messages) {
            val now = System.currentTimeMillis()
            for (m in messages) {
                val text = m.text.trim()
                if (text.isEmpty()) continue
                val key = "${m.receivedAtMs}:${m.user}:${text}"
                if (seen[key] == true) continue
                seen[key] = true

                val lane = pickLane(now)
                active += Bullet(
                    key = key,
                    lane = lane,
                    user = m.user.trim(),
                    text = text,
                )
            }

            // Keep seen map bounded (best-effort).
            if (seen.size > 256) {
                val drop = seen.keys.take(seen.size - 200)
                for (k in drop) seen.remove(k)
            }
        }

        // Draw/animate bullets.
        for (b in active) {
            DanmakuBullet(
                b = b,
                laneHeightPx = lineHeightPx,
                containerWidthPx = widthPx,
                fontSize = fontSize,
                onDone = { doneKey ->
                    val idx = active.indexOfFirst { it.key == doneKey }
                    if (idx >= 0) active.removeAt(idx)
                },
            )
        }
    }
}

private data class Bullet(
    val key: String,
    val lane: Int,
    val user: String,
    val text: String,
)

@Composable
private fun DanmakuBullet(
    b: Bullet,
    laneHeightPx: Float,
    containerWidthPx: Float,
    fontSize: androidx.compose.ui.unit.TextUnit,
    onDone: (String) -> Unit,
) {
    val density = LocalDensity.current
    var measuredWidthPx by remember(b.key) { mutableFloatStateOf(0f) }
    val x = remember(b.key) { Animatable(containerWidthPx) }

    // Speed: ~140dp/s, clamped duration.
    val speedPxPerSec = with(density) { 140.dp.toPx() }

    LaunchedEffect(b.key, containerWidthPx, measuredWidthPx) {
        if (containerWidthPx <= 0f) return@LaunchedEffect
        // Wait for first measure.
        if (measuredWidthPx <= 0f) return@LaunchedEffect

        val distance = containerWidthPx + measuredWidthPx
        val durationMs = ((distance / speedPxPerSec) * 1000f).roundToInt().coerceIn(4_000, 12_000)
        x.snapTo(containerWidthPx)
        x.animateTo(
            targetValue = -measuredWidthPx,
            animationSpec = tween(durationMillis = durationMs, easing = LinearEasing),
        )
        onDone(b.key)
    }

    val yPx = (b.lane * laneHeightPx).coerceAtLeast(0f)

    // Simple style with shadow to mimic bilibili readability.
    val style = TextStyle(
        color = Color.White,
        fontSize = fontSize,
        fontWeight = FontWeight.SemiBold,
        shadow = Shadow(color = Color.Black.copy(alpha = 0.85f), offset = androidx.compose.ui.geometry.Offset(1f, 1f), blurRadius = 3f),
    )

    Text(
        text = if (b.user.isBlank()) b.text else "${b.user}：${b.text}",
        style = style,
        maxLines = 1,
        overflow = TextOverflow.Clip,
        modifier = Modifier
            .alpha(0.95f)
            .onSizeChanged { measuredWidthPx = it.width.toFloat() }
            .offset { IntOffset(x.value.roundToInt(), yPx.roundToInt()) },
    )
}
