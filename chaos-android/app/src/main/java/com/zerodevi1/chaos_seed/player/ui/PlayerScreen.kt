package com.zerodevi1.chaos_seed.player.ui

import android.content.res.Configuration
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.RadioButton
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.unit.dp
import com.zerodevi1.chaos_seed.player.PlayerEngineType
import com.zerodevi1.chaos_seed.player.PlayerViewModel
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PlayerScreen(
    vm: PlayerViewModel,
    pipMode: Boolean,
    onBack: () -> Unit,
    onEnterPip: () -> Unit,
    onToggleOrientation: () -> Unit,
) {
    val state by vm.state.collectAsState()
    val muted by vm.muted.collectAsState()
    val engineType by vm.engineType.collectAsState()
    val title by vm.liveTitle.collectAsState()
    val liveInfo by vm.liveInfo.collectAsState()
    val variants by vm.variants.collectAsState()
    val selectedVariantId by vm.variantId.collectAsState()
    val lines by vm.lines.collectAsState()
    val lineIndex by vm.lineIndex.collectAsState()
    val danmakuEnabled by vm.danmakuEnabled.collectAsState()
    val danmakuTail by vm.danmakuTail.collectAsState()
    val danmuList by vm.danmuList.collectAsState()
    val settings by vm.settings.collectAsState()

    val snackbarHost = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()

    var showPlaybackSheet by remember { mutableStateOf(false) }
    val playbackSheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    var showDanmuSettings by remember { mutableStateOf(false) } // portrait sheet
    var showDanmuDrawer by remember { mutableStateOf(false) } // landscape drawer
    var controlsVisible by remember { mutableStateOf(true) }

    LaunchedEffect(Unit) {
        vm.snackbar.collect { snackbarHost.showSnackbar(it) }
    }

    LaunchedEffect(
        controlsVisible,
        pipMode,
        state.playing,
        state.buffering,
        state.error,
        showPlaybackSheet,
        showDanmuSettings,
        showDanmuDrawer,
    ) {
        if (pipMode) return@LaunchedEffect
        if (!controlsVisible) return@LaunchedEffect
        if (showPlaybackSheet || showDanmuSettings || showDanmuDrawer) return@LaunchedEffect
        if (!state.playing) return@LaunchedEffect
        if (state.buffering) return@LaunchedEffect
        if (state.error != null) return@LaunchedEffect
        delay(5_000)
        controlsVisible = false
    }

    val cfg = LocalConfiguration.current
    val devicePortrait = cfg.orientation == Configuration.ORIENTATION_PORTRAIT
    LaunchedEffect(devicePortrait) {
        if (devicePortrait) showDanmuDrawer = false
    }

    val titleText = title?.trim().orEmpty().ifBlank { "播放（${engineType.label}）" }
    val subtitleText = liveInfo?.name?.trim()?.takeIf { it.isNotEmpty() }
    val avatarUrl = liveInfo?.avatar?.trim()?.takeIf { it.isNotEmpty() }

    val danmuConfig =
        DanmuRenderConfig(
            fontSizeSp = settings.danmuFontSizeSp,
            opacity = settings.danmuOpacity,
            area = settings.danmuArea,
            speedSeconds = settings.danmuSpeedSeconds,
            strokeWidthDp = settings.danmuStrokeWidthDp,
        )

    Box(modifier = Modifier.fillMaxSize()) {
        if (devicePortrait) {
            PortraitRoomScreen(
                state = state,
                title = titleText,
                subtitle = null,
                avatarUrl = avatarUrl,
                pipMode = pipMode,
                controlsVisible = controlsVisible,
                muted = muted,
                danmakuEnabled = danmakuEnabled,
                danmakuTail = danmakuTail,
                danmuList = danmuList,
                danmuConfig = danmuConfig,
                onBack = onBack,
                onEnterPip = onEnterPip,
                onToggleOrientation = onToggleOrientation,
                onToggleMute = { vm.toggleMute() },
                onToggleDanmaku = { vm.toggleDanmakuEnabled() },
                onOpenPlaybackSettings = {
                    controlsVisible = true
                    showPlaybackSheet = true
                },
                onOpenDanmuSettings = {
                    controlsVisible = true
                    showDanmuSettings = true
                },
                onShowUnsupportedSend = showUnsupportedSend(scope, snackbarHost),
                onToggleControls = { controlsVisible = !controlsVisible },
                onDoubleTapTogglePlay = {
                    controlsVisible = true
                    vm.togglePlayPause()
                },
                onSurfaceReady = { vm.attachSurface(it) },
                onSurfaceDestroyed = { vm.detachSurface() },
            )
        } else {
            LandscapeRoomScreen(
                state = state,
                title = titleText,
                subtitle = subtitleText,
                avatarUrl = avatarUrl,
                pipMode = pipMode,
                controlsVisible = controlsVisible,
                muted = muted,
                danmakuEnabled = danmakuEnabled,
                danmakuTail = danmakuTail,
                danmuConfig = danmuConfig,
                onBack = onBack,
                onEnterPip = onEnterPip,
                onToggleOrientation = onToggleOrientation,
                onToggleMute = { vm.toggleMute() },
                onToggleDanmaku = { vm.toggleDanmakuEnabled() },
                onTogglePlay = { vm.togglePlayPause() },
                onReconnect = { vm.reconnect() },
                onOpenPlaybackSettings = {
                    controlsVisible = true
                    showPlaybackSheet = true
                },
                onOpenDanmuSettings = {
                    controlsVisible = true
                    showDanmuDrawer = true
                },
                onToggleControls = {
                    if (!showDanmuDrawer) controlsVisible = !controlsVisible
                },
                onDoubleTapTogglePlay = {
                    controlsVisible = true
                    vm.togglePlayPause()
                },
                onSurfaceReady = { vm.attachSurface(it) },
                onSurfaceDestroyed = { vm.detachSurface() },
            )
        }

        SnackbarHost(
            hostState = snackbarHost,
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .navigationBarsPadding()
                .padding(16.dp),
        )

        DanmuSettingsDrawer(
            visible = !pipMode && !devicePortrait && showDanmuDrawer,
            settings = settings,
            onDismiss = { showDanmuDrawer = false },
            onSetFontSizeSp = { vm.setDanmuFontSizeSp(it) },
            onSetOpacity = { vm.setDanmuOpacity(it) },
            onSetArea = { vm.setDanmuArea(it) },
            onSetSpeedSeconds = { vm.setDanmuSpeedSeconds(it) },
            onSetStrokeWidthDp = { vm.setDanmuStrokeWidthDp(it) },
            onSetBlockWordsEnabled = { vm.setDanmuBlockWordsEnabled(it) },
            onSaveBlockWordsRaw = { vm.setDanmuBlockWordsRaw(it) },
            onClearBlockWordsRaw = { vm.setDanmuBlockWordsRaw("") },
            onReset = { vm.resetDanmuSettingsToDefaults() },
        )
    }

    DanmuSettingsBottomSheet(
        show = !pipMode && devicePortrait && showDanmuSettings,
        settings = settings,
        onDismiss = { showDanmuSettings = false },
        onSetFontSizeSp = { vm.setDanmuFontSizeSp(it) },
        onSetOpacity = { vm.setDanmuOpacity(it) },
        onSetArea = { vm.setDanmuArea(it) },
        onSetSpeedSeconds = { vm.setDanmuSpeedSeconds(it) },
        onSetStrokeWidthDp = { vm.setDanmuStrokeWidthDp(it) },
        onSetBlockWordsEnabled = { vm.setDanmuBlockWordsEnabled(it) },
        onSaveBlockWordsRaw = { vm.setDanmuBlockWordsRaw(it) },
        onClearBlockWordsRaw = { vm.setDanmuBlockWordsRaw("") },
        onReset = { vm.resetDanmuSettingsToDefaults() },
    )

    if (showPlaybackSheet && !pipMode) {
        ModalBottomSheet(
            onDismissRequest = { showPlaybackSheet = false },
            sheetState = playbackSheetState,
        ) {
            PlaybackSettingsSheet(
                current = engineType,
                onPick = { picked ->
                    showPlaybackSheet = false
                    vm.switchEngine(picked)
                },
                danmakuEnabled = danmakuEnabled,
                onDanmakuEnabledChange = { vm.setDanmakuEnabled(it) },
                variants = variants,
                selectedVariantId = selectedVariantId,
                onPickVariant = { id ->
                    showPlaybackSheet = false
                    vm.switchVariant(id)
                },
                lines = lines,
                lineIndex = lineIndex,
                onPickLine = { idx ->
                    showPlaybackSheet = false
                    vm.switchLine(idx)
                },
            )
        }
    }
}

private fun showUnsupportedSend(
    scope: CoroutineScope,
    host: SnackbarHostState,
): () -> Unit {
    return { scope.launch { host.showSnackbar("暂不支持发送弹幕") } }
}

@Composable
private fun PlaybackSettingsSheet(
    current: PlayerEngineType,
    onPick: (PlayerEngineType) -> Unit,
    danmakuEnabled: Boolean,
    onDanmakuEnabledChange: (Boolean) -> Unit,
    variants: List<com.zerodevi1.chaos_seed.core.model.LivestreamVariant>,
    selectedVariantId: String?,
    onPickVariant: (String) -> Unit,
    lines: List<String>,
    lineIndex: Int,
    onPickLine: (Int) -> Unit,
) {
    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("播放设置")
        Text("播放引擎")
        PlayerEngineType.entries.forEach { t ->
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = 2.dp),
            ) {
                RadioButton(selected = t == current, onClick = { onPick(t) })
                Text(t.label, modifier = Modifier.padding(start = 8.dp))
            }
        }
        Text("提示：若 MPV 初始化或打开失败，会自动回退到 EXO。")

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("弹幕")
            Switch(checked = danmakuEnabled, onCheckedChange = onDanmakuEnabledChange)
        }

        if (variants.isNotEmpty()) {
            Text("清晰度")
            variants.forEach { v ->
                val selected = v.id == selectedVariantId
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 2.dp),
                ) {
                    RadioButton(selected = selected, onClick = { onPickVariant(v.id) })
                    Text("${v.label}（${v.quality}）", modifier = Modifier.padding(start = 8.dp))
                }
            }
        }

        if (lines.isNotEmpty()) {
            Text("线路")
            lines.forEachIndexed { idx, _ ->
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 2.dp),
                ) {
                    RadioButton(selected = idx == lineIndex, onClick = { onPickLine(idx) })
                    Text("线路 ${idx + 1}", modifier = Modifier.padding(start = 8.dp))
                }
            }
        }
    }
}
