package com.zerodevi1.chaos_seed.player.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableFloatState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import com.zerodevi1.chaos_seed.settings.AppSettings
import kotlin.math.abs
import kotlin.math.roundToInt

private val AREA_OPTIONS = listOf(0.25f, 0.5f, 0.75f, 1.0f)

@Composable
fun DanmuSettingsDrawer(
    visible: Boolean,
    settings: AppSettings,
    onDismiss: () -> Unit,
    onSetFontSizeSp: (Float) -> Unit,
    onSetOpacity: (Float) -> Unit,
    onSetArea: (Float) -> Unit,
    onSetSpeedSeconds: (Int) -> Unit,
    onSetStrokeWidthDp: (Float) -> Unit,
    onSetBlockWordsEnabled: (Boolean) -> Unit,
    onSaveBlockWordsRaw: (String) -> Unit,
    onClearBlockWordsRaw: () -> Unit,
    onReset: () -> Unit,
    modifier: Modifier = Modifier,
) {
    AnimatedVisibility(
        visible = visible,
        enter = fadeIn(),
        exit = fadeOut(),
    ) {
        Box(modifier = Modifier.fillMaxSize().then(modifier)) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(Color.Black.copy(alpha = 0.45f))
                    .clickable(onClick = onDismiss),
            )

            AnimatedVisibility(
                visible = visible,
                enter = slideInHorizontally { it } + fadeIn(),
                exit = slideOutHorizontally { it } + fadeOut(),
                modifier = Modifier.align(Alignment.CenterEnd),
            ) {
                SurfacePanel(
                    settings = settings,
                    onClose = onDismiss,
                    onSetFontSizeSp = onSetFontSizeSp,
                    onSetOpacity = onSetOpacity,
                    onSetArea = onSetArea,
                    onSetSpeedSeconds = onSetSpeedSeconds,
                    onSetStrokeWidthDp = onSetStrokeWidthDp,
                    onSetBlockWordsEnabled = onSetBlockWordsEnabled,
                    onSaveBlockWordsRaw = onSaveBlockWordsRaw,
                    onClearBlockWordsRaw = onClearBlockWordsRaw,
                    onReset = onReset,
                    modifier = Modifier
                        .fillMaxHeight()
                        .width(360.dp)
                        .padding(start = 18.dp)
                        .background(Color.Transparent),
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DanmuSettingsBottomSheet(
    show: Boolean,
    settings: AppSettings,
    onDismiss: () -> Unit,
    onSetFontSizeSp: (Float) -> Unit,
    onSetOpacity: (Float) -> Unit,
    onSetArea: (Float) -> Unit,
    onSetSpeedSeconds: (Int) -> Unit,
    onSetStrokeWidthDp: (Float) -> Unit,
    onSetBlockWordsEnabled: (Boolean) -> Unit,
    onSaveBlockWordsRaw: (String) -> Unit,
    onClearBlockWordsRaw: () -> Unit,
    onReset: () -> Unit,
) {
    if (!show) return
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
    ) {
        SurfacePanel(
            settings = settings,
            onClose = onDismiss,
            onSetFontSizeSp = onSetFontSizeSp,
            onSetOpacity = onSetOpacity,
            onSetArea = onSetArea,
            onSetSpeedSeconds = onSetSpeedSeconds,
            onSetStrokeWidthDp = onSetStrokeWidthDp,
            onSetBlockWordsEnabled = onSetBlockWordsEnabled,
            onSaveBlockWordsRaw = onSaveBlockWordsRaw,
            onClearBlockWordsRaw = onClearBlockWordsRaw,
            onReset = onReset,
            modifier = Modifier.padding(bottom = 12.dp),
        )
    }
}

@Composable
private fun SurfacePanel(
    settings: AppSettings,
    onClose: () -> Unit,
    onSetFontSizeSp: (Float) -> Unit,
    onSetOpacity: (Float) -> Unit,
    onSetArea: (Float) -> Unit,
    onSetSpeedSeconds: (Int) -> Unit,
    onSetStrokeWidthDp: (Float) -> Unit,
    onSetBlockWordsEnabled: (Boolean) -> Unit,
    onSaveBlockWordsRaw: (String) -> Unit,
    onClearBlockWordsRaw: () -> Unit,
    onReset: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val density = LocalDensity.current
    val scroll = rememberScrollState()

    val initialAreaIdx = remember(settings.danmuArea) { closestAreaIndex(settings.danmuArea) }
    val areaIdx = remember { mutableIntStateOf(initialAreaIdx) }
    val opacity = rememberSliderValue(settings.danmuOpacity)
    val fontSize = rememberSliderValue(settings.danmuFontSizeSp)
    val speedSeconds = remember { mutableIntStateOf(settings.danmuSpeedSeconds.coerceIn(4, 16)) }
    val strokeWidth = rememberSliderValue(settings.danmuStrokeWidthDp)
    var blockWordsText by rememberSaveable { mutableStateOf(settings.danmuBlockWordsRaw) }

    LaunchedEffect(settings.danmuBlockWordsRaw) {
        if (blockWordsText.isBlank()) blockWordsText = settings.danmuBlockWordsRaw
    }

    androidx.compose.material3.Surface(
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(topStart = 18.dp, bottomStart = 18.dp),
        modifier = modifier.fillMaxWidth(),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(scroll)
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text("弹幕设置", style = MaterialTheme.typography.titleMedium)
                FilledTonalIconButton(onClick = onClose, modifier = Modifier.size(40.dp)) {
                    Icon(Icons.Outlined.Close, contentDescription = "Close")
                }
            }

            SettingSliderRow(
                title = "显示区域",
                valueLabel = areaLabel(AREA_OPTIONS[areaIdx.intValue]),
            ) {
                Slider(
                    value = areaIdx.intValue.toFloat(),
                    onValueChange = { areaIdx.intValue = it.roundToInt().coerceIn(0, 3) },
                    valueRange = 0f..3f,
                    steps = 2,
                    onValueChangeFinished = { onSetArea(AREA_OPTIONS[areaIdx.intValue]) },
                )
            }

            SettingSliderRow(
                title = "不透明度",
                valueLabel = "${(opacity.floatValue.coerceIn(0.2f, 1.0f) * 100f).roundToInt()}%",
            ) {
                Slider(
                    value = opacity.floatValue,
                    onValueChange = { opacity.floatValue = it },
                    valueRange = 0.2f..1.0f,
                    onValueChangeFinished = { onSetOpacity(opacity.floatValue) },
                )
            }

            SettingSliderRow(
                title = "弹幕速度",
                valueLabel = speedLabel(speedSeconds.intValue),
                supporting = "${speedSeconds.intValue} 秒",
            ) {
                Slider(
                    value = speedSeconds.intValue.toFloat(),
                    onValueChange = { speedSeconds.intValue = it.roundToInt().coerceIn(4, 16) },
                    valueRange = 4f..16f,
                    steps = 11,
                    onValueChangeFinished = { onSetSpeedSeconds(speedSeconds.intValue) },
                )
            }

            SettingSliderRow(
                title = "字体大小",
                valueLabel = String.format("%.1f倍", (fontSize.floatValue.coerceIn(12f, 32f) / 18f)),
            ) {
                Slider(
                    value = fontSize.floatValue,
                    onValueChange = { fontSize.floatValue = it },
                    valueRange = 12f..32f,
                    steps = 19,
                    onValueChangeFinished = { onSetFontSizeSp(fontSize.floatValue) },
                )
            }

            val strokePx = with(density) { strokeWidth.floatValue.coerceIn(0f, 4f).dp.toPx() }
            SettingSliderRow(
                title = "描边宽度",
                valueLabel = String.format("%.1fpx", strokePx),
            ) {
                Slider(
                    value = strokeWidth.floatValue,
                    onValueChange = { strokeWidth.floatValue = it },
                    valueRange = 0f..4f,
                    onValueChangeFinished = { onSetStrokeWidthDp(strokeWidth.floatValue) },
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text("弹幕屏蔽词", style = MaterialTheme.typography.titleSmall)
                Switch(
                    checked = settings.danmuBlockWordsEnabled,
                    onCheckedChange = onSetBlockWordsEnabled,
                )
            }

            Text("每行一个关键词；命中则不显示该条弹幕", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)

            OutlinedTextField(
                value = blockWordsText,
                onValueChange = { blockWordsText = it },
                minLines = 4,
                maxLines = 8,
                label = { Text("屏蔽词列表") },
                modifier = Modifier.fillMaxWidth(),
            )

            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                FilledTonalButton(onClick = { onSaveBlockWordsRaw(blockWordsText) }) { Text("保存") }
                OutlinedButton(
                    onClick = {
                        blockWordsText = ""
                        onClearBlockWordsRaw()
                    },
                ) { Text("清空") }
                Spacer(modifier = Modifier.weight(1f))
                Button(
                    onClick = {
                        blockWordsText = ""
                        areaIdx.intValue = closestAreaIndex(0.6f)
                        opacity.floatValue = 0.85f
                        fontSize.floatValue = 18f
                        speedSeconds.intValue = 8
                        strokeWidth.floatValue = 1.0f
                        onReset()
                    },
                ) { Text("恢复默认") }
            }
        }
    }
}

@Composable
private fun SettingSliderRow(
    title: String,
    valueLabel: String,
    supporting: String? = null,
    content: @Composable () -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(title, style = MaterialTheme.typography.titleSmall)
            Text(valueLabel, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (!supporting.isNullOrBlank()) {
            Text(supporting, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        content()
    }
}

@Composable
private fun rememberSliderValue(initial: Float): MutableFloatState {
    val state = remember { mutableFloatStateOf(initial) }
    LaunchedEffect(initial) { state.floatValue = initial }
    return state
}

private fun closestAreaIndex(v: Float): Int {
    val x = v.coerceIn(0.25f, 1.0f)
    var bestIdx = 0
    var bestDist = Float.MAX_VALUE
    for (i in AREA_OPTIONS.indices) {
        val d = abs(AREA_OPTIONS[i] - x)
        if (d < bestDist) {
            bestDist = d
            bestIdx = i
        }
    }
    return bestIdx
}

private fun areaLabel(v: Float): String =
    when (v) {
        0.25f -> "1/4屏"
        0.5f -> "1/2屏"
        0.75f -> "3/4屏"
        else -> "全屏"
    }

private fun speedLabel(seconds: Int): String {
    val s = seconds.coerceIn(4, 16)
    return when {
        s <= 6 -> "快"
        s <= 9 -> "中"
        else -> "慢"
    }
}
