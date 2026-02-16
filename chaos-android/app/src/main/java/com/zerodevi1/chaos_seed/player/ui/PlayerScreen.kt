package com.zerodevi1.chaos_seed.player.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.outlined.ChatBubbleOutline
import androidx.compose.material.icons.outlined.ChatBubble
import androidx.compose.material.icons.outlined.Pause
import androidx.compose.material.icons.outlined.PictureInPictureAlt
import androidx.compose.material.icons.outlined.PlayArrow
import androidx.compose.material.icons.outlined.ScreenRotation
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.VolumeOff
import androidx.compose.material.icons.outlined.VolumeUp
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.IconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.RadioButton
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.unit.dp
import com.zerodevi1.chaos_seed.player.PlayerEngineType
import com.zerodevi1.chaos_seed.player.PlayerViewModel
import kotlinx.coroutines.delay
import android.content.res.Configuration
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch

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
    val variants by vm.variants.collectAsState()
    val selectedVariantId by vm.variantId.collectAsState()
    val lines by vm.lines.collectAsState()
    val lineIndex by vm.lineIndex.collectAsState()
    val danmakuEnabled by vm.danmakuEnabled.collectAsState()
    val danmakuTail by vm.danmakuTail.collectAsState()
    val snackbarHost = remember { SnackbarHostState() }

    var showSheet by remember { mutableStateOf(false) }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var controlsVisible by remember { mutableStateOf(true) }

    LaunchedEffect(Unit) {
        vm.snackbar.collect { snackbarHost.showSnackbar(it) }
    }

    LaunchedEffect(controlsVisible, pipMode, state.playing, state.buffering, state.error) {
        if (pipMode) return@LaunchedEffect
        if (!controlsVisible) return@LaunchedEffect
        if (!state.playing) return@LaunchedEffect
        if (state.buffering) return@LaunchedEffect
        if (state.error != null) return@LaunchedEffect
        delay(5_000)
        controlsVisible = false
    }

    val cfg = LocalConfiguration.current
    val devicePortrait = cfg.orientation == Configuration.ORIENTATION_PORTRAIT
    val w = state.videoWidth
    val h = state.videoHeight
    val videoAspect = remember(w, h) { if (w > 0 && h > 0) (w.toFloat() / h.toFloat()) else (16f / 9f) }
    val useCenteredPlayerArea = devicePortrait && videoAspect >= 1.2f
    val chromeBottomPad = 56.dp + 8.dp

    if (devicePortrait) {
        Scaffold(
            containerColor = Color.Black,
            topBar = {
                if (!pipMode && controlsVisible) {
                    TopAppBar(
                        title = {
                            val t = (title ?: "").trim()
                            Text(text = if (t.isEmpty()) "播放（${engineType.label}）" else t, maxLines = 1)
                        },
                        navigationIcon = {
                            FilledTonalIconButton(onClick = onBack) {
                                Icon(Icons.AutoMirrored.Outlined.ArrowBack, contentDescription = "Back")
                            }
                        },
                        actions = {
                            FilledTonalIconButton(onClick = { showSheet = true }) {
                                Icon(Icons.Outlined.Settings, contentDescription = "Playback settings")
                            }
                            FilledTonalIconButton(onClick = onEnterPip) {
                                Icon(Icons.Outlined.PictureInPictureAlt, contentDescription = "PiP")
                            }
                        },
                        colors = TopAppBarDefaults.topAppBarColors(
                            containerColor = Color.Black.copy(alpha = 0.35f),
                            titleContentColor = Color.White,
                            actionIconContentColor = Color.White,
                            navigationIconContentColor = Color.White,
                        ),
                        modifier = Modifier.statusBarsPadding(),
                    )
                }
            },
            bottomBar = {
                if (!pipMode && controlsVisible) {
                    PortraitBottomControls(
                        playing = state.playing,
                        muted = muted,
                        buffering = state.buffering,
                        error = state.error,
                        onTogglePlay = { vm.togglePlayPause() },
                        onToggleMute = { vm.toggleMute() },
                        modifier = Modifier
                            .fillMaxWidth()
                            .navigationBarsPadding()
                            .padding(horizontal = 16.dp, vertical = 12.dp),
                    )
                }
            },
            snackbarHost = {
                SnackbarHost(
                    hostState = snackbarHost,
                    modifier = Modifier
                        .fillMaxWidth()
                        .navigationBarsPadding()
                        .padding(16.dp),
                )
            },
        ) { inner ->
            Surface(modifier = Modifier.fillMaxSize(), color = Color.Black) {
                Box(modifier = Modifier.fillMaxSize().padding(inner)) {
                    val videoModifier =
                        if (useCenteredPlayerArea) {
                            Modifier
                                .align(Alignment.Center)
                                .fillMaxWidth()
                                .aspectRatio(videoAspect)
                        } else {
                            Modifier.fillMaxSize()
                        }

                    Box(modifier = videoModifier) {
                        PlayerSurface(
                            modifier = Modifier
                                .fillMaxSize()
                                .pointerInput(Unit) {
                                    detectTapGestures(
                                        onTap = { controlsVisible = !controlsVisible },
                                        onDoubleTap = {
                                            controlsVisible = true
                                            vm.togglePlayPause()
                                        },
                                    )
                                },
                            onSurfaceReady = { vm.attachSurface(it) },
                            onSurfaceDestroyed = { vm.detachSurface() },
                        )

                        if (!pipMode && controlsVisible) {
                            VideoQuickActions(
                                danmakuEnabled = danmakuEnabled,
                                onToggleDanmaku = { vm.toggleDanmakuEnabled() },
                                onToggleOrientation = onToggleOrientation,
                                modifier = Modifier
                                    .align(Alignment.TopEnd)
                                    .padding(top = 8.dp),
                            )
                        }

                        DanmakuFlyingOverlay(
                            messages = danmakuTail,
                            enabled = danmakuEnabled,
                            modifier = Modifier.fillMaxSize(),
                        )
                    }
                }
            }
        }
    } else {
        Surface(modifier = Modifier.fillMaxSize(), color = Color.Black) {
            Box(modifier = Modifier.fillMaxSize()) {
                // video layer
                Box(modifier = Modifier.fillMaxSize()) {
                    PlayerSurface(
                        modifier = Modifier
                            .fillMaxSize()
                            .pointerInput(Unit) {
                                detectTapGestures(
                                    onTap = { controlsVisible = !controlsVisible },
                                    onDoubleTap = {
                                        controlsVisible = true
                                        vm.togglePlayPause()
                                    },
                                )
                            },
                        onSurfaceReady = { vm.attachSurface(it) },
                        onSurfaceDestroyed = { vm.detachSurface() },
                    )

                    if (!pipMode && controlsVisible) {
                        LiveControlsOverlay(
                            playing = state.playing,
                            muted = muted,
                            buffering = state.buffering,
                            error = state.error,
                            onTogglePlay = { vm.togglePlayPause() },
                            onToggleMute = { vm.toggleMute() },
                            modifier = Modifier.fillMaxSize(),
                        )
                    }

                    if (!pipMode && controlsVisible) {
                        VideoQuickActions(
                            danmakuEnabled = danmakuEnabled,
                            onToggleDanmaku = { vm.toggleDanmakuEnabled() },
                            onToggleOrientation = onToggleOrientation,
                            // Place below the top title bar to avoid overlap.
                            modifier = Modifier
                                .align(Alignment.TopEnd)
                                .statusBarsPadding()
                                .padding(top = chromeBottomPad),
                        )
                    }

                    DanmakuFlyingOverlay(
                        messages = danmakuTail,
                        enabled = danmakuEnabled,
                        modifier = Modifier.fillMaxSize(),
                    )
                }

                if (!pipMode && controlsVisible) {
                    TopAppBar(
                        title = {
                            val t = (title ?: "").trim()
                            Text(text = if (t.isEmpty()) "播放（${engineType.label}）" else t, maxLines = 1)
                        },
                        navigationIcon = {
                            FilledTonalIconButton(onClick = onBack) {
                                Icon(Icons.AutoMirrored.Outlined.ArrowBack, contentDescription = "Back")
                            }
                        },
                        actions = {
                            FilledTonalIconButton(onClick = { showSheet = true }) {
                                Icon(Icons.Outlined.Settings, contentDescription = "Playback settings")
                            }
                            FilledTonalIconButton(onClick = onEnterPip) {
                                Icon(Icons.Outlined.PictureInPictureAlt, contentDescription = "PiP")
                            }
                        },
                        colors = TopAppBarDefaults.topAppBarColors(
                            containerColor = Color.Black.copy(alpha = 0.35f),
                            titleContentColor = Color.White,
                            actionIconContentColor = Color.White,
                            navigationIconContentColor = Color.White,
                        ),
                        modifier = Modifier
                            .align(Alignment.TopCenter)
                            .statusBarsPadding(),
                    )
                }

                SnackbarHost(
                    hostState = snackbarHost,
                    modifier = Modifier
                        .align(Alignment.BottomCenter)
                        .navigationBarsPadding()
                        .padding(16.dp),
                )
            }
        }
    }

    if (showSheet && !pipMode) {
        ModalBottomSheet(
            onDismissRequest = { showSheet = false },
            sheetState = sheetState,
        ) {
            PlaybackSettingsSheet(
                current = engineType,
                onPick = { picked ->
                    showSheet = false
                    vm.switchEngine(picked)
                },
                danmakuEnabled = danmakuEnabled,
                onDanmakuEnabledChange = { vm.setDanmakuEnabled(it) },
                variants = variants,
                selectedVariantId = selectedVariantId,
                onPickVariant = { id ->
                    showSheet = false
                    vm.switchVariant(id)
                },
                lines = lines,
                lineIndex = lineIndex,
                onPickLine = { idx ->
                    showSheet = false
                    vm.switchLine(idx)
                },
            )
        }
    }
}

@Composable
private fun LiveControlsOverlay(
    playing: Boolean,
    muted: Boolean,
    buffering: Boolean,
    error: String?,
    onTogglePlay: () -> Unit,
    onToggleMute: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(modifier = modifier) {
        // Center play/pause (like fig1/fig2)
        FilledTonalIconButton(
            onClick = onTogglePlay,
            modifier = Modifier
                .align(Alignment.Center)
                .padding(16.dp),
        ) {
            Icon(
                imageVector = if (playing) Icons.Outlined.Pause else Icons.Outlined.PlayArrow,
                contentDescription = if (playing) "Pause" else "Play",
            )
        }

        // Bottom pill actions (match previous "暂停/静音" feel)
        Row(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .padding(bottom = 22.dp)
                .background(Color.Black.copy(alpha = 0.35f), RoundedCornerShape(999.dp))
                .padding(horizontal = 10.dp, vertical = 10.dp),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            FilledTonalButton(onClick = onTogglePlay) { Text(if (playing) "暂停" else "播放") }
            FilledTonalButton(onClick = onToggleMute) { Text(if (muted) "取消静音" else "静音") }
        }

        if (error != null || buffering) {
            Column(
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .padding(bottom = 92.dp)
                    .background(Color.Black.copy(alpha = 0.35f), RoundedCornerShape(12.dp))
                    .padding(horizontal = 12.dp, vertical = 10.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                if (error != null) {
                    Text("错误：$error", color = Color.White)
                } else if (buffering) {
                    Text("缓冲中...", color = Color.White)
                }
            }
        }

        // A minimal "right rail" volume affordance to echo fig1/fig2.
        FilledTonalIconButton(
            onClick = onToggleMute,
            modifier = Modifier
                .align(Alignment.CenterEnd)
                .padding(end = 12.dp),
        ) {
            Icon(
                imageVector = if (muted) Icons.Outlined.VolumeOff else Icons.Outlined.VolumeUp,
                contentDescription = if (muted) "Unmute" else "Mute",
            )
        }
    }
}

@Composable
private fun PortraitBottomControls(
    playing: Boolean,
    muted: Boolean,
    buffering: Boolean,
    error: String?,
    onTogglePlay: () -> Unit,
    onToggleMute: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier, verticalArrangement = Arrangement.spacedBy(10.dp)) {
        if (error != null || buffering) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(Color.Black.copy(alpha = 0.35f), RoundedCornerShape(12.dp))
                    .padding(horizontal = 12.dp, vertical = 10.dp),
            ) {
                if (error != null) Text("错误：$error", color = Color.White)
                else if (buffering) Text("缓冲中...", color = Color.White)
            }
        }

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(Color.Black.copy(alpha = 0.35f), RoundedCornerShape(999.dp))
                .padding(horizontal = 10.dp, vertical = 10.dp),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            FilledTonalButton(onClick = onTogglePlay, modifier = Modifier.weight(1f)) {
                Text(if (playing) "暂停" else "播放")
            }
            FilledTonalButton(onClick = onToggleMute, modifier = Modifier.weight(1f)) {
                Text(if (muted) "取消静音" else "静音")
            }
        }
    }
}

@Composable
private fun VideoQuickActions(
    danmakuEnabled: Boolean,
    onToggleDanmaku: () -> Unit,
    onToggleOrientation: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier.padding(10.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        FilledTonalIconButton(onClick = onToggleOrientation) {
            Icon(Icons.Outlined.ScreenRotation, contentDescription = "Rotate")
        }
        FilledTonalIconButton(onClick = onToggleDanmaku) {
            Icon(
                imageVector = if (danmakuEnabled) Icons.Outlined.ChatBubble else Icons.Outlined.ChatBubbleOutline,
                contentDescription = "Danmaku",
            )
        }
    }
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
