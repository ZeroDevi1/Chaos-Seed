package com.zerodevi1.chaos_seed.player.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ChatBubble
import androidx.compose.material.icons.outlined.PictureInPictureAlt
import androidx.compose.material.icons.outlined.PlayArrow
import androidx.compose.material.icons.outlined.Pause
import androidx.compose.material.icons.outlined.Refresh
import androidx.compose.material.icons.outlined.ScreenRotation
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.material.icons.automirrored.outlined.VolumeOff
import androidx.compose.material.icons.automirrored.outlined.VolumeUp
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.unit.dp
import com.zerodevi1.chaos_seed.core.model.DanmakuMessage
import com.zerodevi1.chaos_seed.player.engine.PlayerState

@Composable
fun LandscapeRoomScreen(
    state: PlayerState,
    title: String,
    subtitle: String?,
    avatarUrl: String?,
    pipMode: Boolean,
    controlsVisible: Boolean,
    muted: Boolean,
    danmakuEnabled: Boolean,
    danmakuTail: List<DanmakuMessage>,
    danmuConfig: DanmuRenderConfig,
    onBack: () -> Unit,
    onEnterPip: () -> Unit,
    onToggleOrientation: () -> Unit,
    onToggleMute: () -> Unit,
    onToggleDanmaku: () -> Unit,
    onTogglePlay: () -> Unit,
    onReconnect: () -> Unit,
    onOpenPlaybackSettings: () -> Unit,
    onOpenDanmuSettings: () -> Unit,
    onToggleControls: () -> Unit,
    onDoubleTapTogglePlay: () -> Unit,
    onSurfaceReady: (android.view.Surface) -> Unit,
    onSurfaceDestroyed: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val cfg = LocalConfiguration.current

    Box(modifier = modifier.fillMaxSize().background(Color.Black)) {
        key(cfg.orientation) {
            PlayerSurface(
                modifier = Modifier
                    .fillMaxSize()
                    .pointerInput(Unit) {
                        detectTapGestures(
                            onTap = { onToggleControls() },
                            onDoubleTap = { onDoubleTapTogglePlay() },
                        )
                    },
                onSurfaceReady = onSurfaceReady,
                onSurfaceDestroyed = onSurfaceDestroyed,
            )
        }

        DanmakuFlyingOverlay(
            messages = danmakuTail,
            enabled = !pipMode && danmakuEnabled,
            config = danmuConfig,
            modifier = Modifier.fillMaxSize(),
        )

        if (!pipMode && controlsVisible) {
            TopInfoBar(
                title = title,
                subtitle = subtitle,
                avatarUrl = avatarUrl,
                onBack = onBack,
                actions = {
                    FilledTonalIconButton(onClick = onOpenPlaybackSettings) {
                        Icon(Icons.Outlined.Settings, contentDescription = "Playback settings")
                    }
                    FilledTonalIconButton(onClick = onEnterPip) {
                        Icon(Icons.Outlined.PictureInPictureAlt, contentDescription = "PiP")
                    }
                },
                modifier = Modifier
                    .align(Alignment.TopStart)
                    .statusBarsPadding()
                    .padding(start = 10.dp, top = 8.dp)
                    .fillMaxWidth(0.7f),
            )

            BottomControlRow(
                modifier = Modifier
                    .align(Alignment.BottomStart)
                    .navigationBarsPadding()
                    .padding(start = 10.dp, bottom = 12.dp),
            ) {
                FilledTonalIconButton(onClick = onTogglePlay, modifier = Modifier.size(42.dp)) {
                    Icon(
                        imageVector = if (state.playing) Icons.Outlined.Pause else Icons.Outlined.PlayArrow,
                        contentDescription = "Play/Pause",
                    )
                }
                FilledTonalIconButton(onClick = onReconnect, modifier = Modifier.size(42.dp)) {
                    Icon(Icons.Outlined.Refresh, contentDescription = "Reconnect")
                }
                FilledTonalIconButton(onClick = onToggleMute, modifier = Modifier.size(42.dp)) {
                    Icon(
                        imageVector = if (muted) Icons.AutoMirrored.Outlined.VolumeOff else Icons.AutoMirrored.Outlined.VolumeUp,
                        contentDescription = "Mute",
                    )
                }
                FilledTonalIconButton(onClick = onToggleOrientation, modifier = Modifier.size(42.dp)) {
                    Icon(Icons.Outlined.ScreenRotation, contentDescription = "Rotate")
                }
                FilledTonalIconButton(onClick = onToggleDanmaku, modifier = Modifier.size(42.dp)) {
                    Icon(Icons.Outlined.ChatBubble, contentDescription = "Danmu toggle")
                }
                FilledTonalIconButton(onClick = onOpenDanmuSettings, modifier = Modifier.size(42.dp)) {
                    Icon(Icons.Outlined.Tune, contentDescription = "Danmu settings")
                }
            }

            RailButtonColumn(
                modifier = Modifier
                    .align(Alignment.BottomEnd)
                    .navigationBarsPadding()
                    .padding(end = 10.dp, bottom = 12.dp),
            ) {
                FilledTonalIconButton(onClick = onOpenPlaybackSettings) {
                    Icon(Icons.Outlined.Settings, contentDescription = "More")
                }
            }
        }

    }
}
