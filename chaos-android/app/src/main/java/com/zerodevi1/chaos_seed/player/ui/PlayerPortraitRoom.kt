package com.zerodevi1.chaos_seed.player.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.MoreVert
import androidx.compose.material.icons.outlined.CardGiftcard
import androidx.compose.material.icons.outlined.EmojiEmotions
import androidx.compose.material.icons.outlined.PictureInPictureAlt
import androidx.compose.material.icons.outlined.ScreenRotation
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.material.icons.outlined.ChatBubble
import androidx.compose.material.icons.outlined.ChatBubbleOutline
import androidx.compose.material.icons.automirrored.outlined.VolumeOff
import androidx.compose.material.icons.automirrored.outlined.VolumeUp
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.unit.dp
import coil.compose.AsyncImage
import com.zerodevi1.chaos_seed.core.model.DanmakuMessage
import com.zerodevi1.chaos_seed.player.engine.PlayerState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue

@Composable
fun PortraitRoomScreen(
    state: PlayerState,
    title: String,
    subtitle: String?,
    avatarUrl: String?,
    pipMode: Boolean,
    controlsVisible: Boolean,
    muted: Boolean,
    danmakuEnabled: Boolean,
    danmakuTail: List<DanmakuMessage>,
    danmuList: List<DanmakuMessage>,
    danmuConfig: DanmuRenderConfig,
    onBack: () -> Unit,
    onEnterPip: () -> Unit,
    onToggleOrientation: () -> Unit,
    onToggleMute: () -> Unit,
    onToggleDanmaku: () -> Unit,
    onOpenPlaybackSettings: () -> Unit,
    onOpenDanmuSettings: () -> Unit,
    onShowUnsupportedSend: () -> Unit,
    onToggleControls: () -> Unit,
    onDoubleTapTogglePlay: () -> Unit,
    onSurfaceReady: (android.view.Surface) -> Unit,
    onSurfaceDestroyed: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val cfg = LocalConfiguration.current
    val chatHeight =
        remember(cfg.screenHeightDp) {
            val h = (cfg.screenHeightDp.dp * 0.32f)
            h.coerceIn(180.dp, 320.dp)
        }

    Column(modifier = modifier.fillMaxSize().background(Color.Black)) {
        Box(
            modifier = if (pipMode) Modifier.fillMaxSize() else Modifier.fillMaxWidth().weight(1f),
        ) {
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
                    actions = { },
                    modifier = Modifier
                        .align(Alignment.TopStart)
                        .statusBarsPadding()
                        .padding(start = 10.dp, top = 8.dp)
                        .fillMaxWidth(0.72f),
                )

                PortraitTopActions(
                    muted = muted,
                    danmakuEnabled = danmakuEnabled,
                    onToggleDanmaku = onToggleDanmaku,
                    onOpenMenuPlaybackSettings = onOpenPlaybackSettings,
                    onOpenMenuDanmuSettings = onOpenDanmuSettings,
                    onOpenMenuPip = onEnterPip,
                    onOpenMenuRotate = onToggleOrientation,
                    onOpenMenuToggleMute = onToggleMute,
                    modifier = Modifier
                        .align(Alignment.TopEnd)
                        .statusBarsPadding()
                        .padding(top = 8.dp, end = 10.dp),
                )
            }
        }

        if (!pipMode) {
            ChatList(
                messages = danmuList,
                danmakuEnabled = danmakuEnabled,
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = 160.dp)
                    .height(chatHeight)
                    .padding(horizontal = 10.dp, vertical = 8.dp),
            )

            DanmuInputBarPlaceholder(
                onClickInput = onShowUnsupportedSend,
                onClickEmoji = onShowUnsupportedSend,
                onClickGift = onShowUnsupportedSend,
                modifier = Modifier
                    .fillMaxWidth()
                    .navigationBarsPadding()
                    .padding(horizontal = 10.dp, vertical = 10.dp),
            )
        } else {
            Spacer(modifier = Modifier.weight(1f))
        }
    }
}

@Composable
private fun ChatList(
    messages: List<DanmakuMessage>,
    danmakuEnabled: Boolean,
    modifier: Modifier = Modifier,
) {
    val items = messages.takeLast(200)
    Surface(
        modifier = modifier.clip(RoundedCornerShape(16.dp)),
        color = Color.Black.copy(alpha = 0.25f),
    ) {
        if (!danmakuEnabled) {
            Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("弹幕已关闭", color = Color.White.copy(alpha = 0.75f))
            }
            return@Surface
        }
        if (items.isEmpty()) {
            Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("暂无弹幕", color = Color.White.copy(alpha = 0.75f))
            }
            return@Surface
        }
        LazyColumn(
            reverseLayout = true,
            modifier = Modifier.fillMaxSize().padding(10.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(items.asReversed(), key = { "${it.receivedAtMs}:${it.user}:${it.text}" }) { m ->
                ChatBubble(message = m)
            }
        }
    }
}

@Composable
private fun ChatBubble(
    message: DanmakuMessage,
    modifier: Modifier = Modifier,
) {
    val text = message.text.trim()
    if (text.isEmpty() && message.imageUrl.isNullOrBlank()) return
    Surface(
        color = Color(0xFF0C0C0C).copy(alpha = 0.55f),
        shape = RoundedCornerShape(14.dp),
        modifier = modifier.fillMaxWidth(),
    ) {
        Column(modifier = Modifier.padding(horizontal = 12.dp, vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            val user = message.user.trim()
            if (user.isNotEmpty()) {
                Text(user, color = Color.White.copy(alpha = 0.85f), style = MaterialTheme.typography.labelMedium)
            }
            if (!message.imageUrl.isNullOrBlank()) {
                AsyncImage(
                    model = message.imageUrl,
                    contentDescription = null,
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(10.dp)),
                )
            }
            if (text.isNotEmpty()) {
                Text(
                    text = if (user.isEmpty()) text else "：$text",
                    color = Color.White,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
    }
}

@Composable
private fun DanmuInputBarPlaceholder(
    onClickInput: () -> Unit,
    onClickEmoji: () -> Unit,
    onClickGift: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier.clip(RoundedCornerShape(999.dp)),
        color = Color.Black.copy(alpha = 0.35f),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 14.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Surface(
                modifier = Modifier
                    .weight(1f)
                    .clip(RoundedCornerShape(999.dp))
                    .background(Color.White.copy(alpha = 0.06f))
                    .padding(horizontal = 12.dp, vertical = 10.dp),
                color = Color.Transparent,
                onClick = onClickInput,
            ) {
                Text("弹幕支持下~", color = Color.White.copy(alpha = 0.75f))
            }

            FilledTonalIconButton(onClick = onClickEmoji, modifier = Modifier.size(40.dp)) {
                Icon(Icons.Outlined.EmojiEmotions, contentDescription = "Emoji")
            }
            FilledTonalIconButton(onClick = onClickGift, modifier = Modifier.size(40.dp)) {
                Icon(Icons.Outlined.CardGiftcard, contentDescription = "Gift")
            }
        }
    }
}

@Composable
private fun PortraitTopActions(
    muted: Boolean,
    danmakuEnabled: Boolean,
    onToggleDanmaku: () -> Unit,
    onOpenMenuPlaybackSettings: () -> Unit,
    onOpenMenuDanmuSettings: () -> Unit,
    onOpenMenuPip: () -> Unit,
    onOpenMenuRotate: () -> Unit,
    onOpenMenuToggleMute: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var menu by remember { mutableStateOf(false) }
    Row(modifier = modifier, horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        FilledTonalIconButton(onClick = onToggleDanmaku, modifier = Modifier.size(42.dp)) {
            Icon(
                imageVector = if (danmakuEnabled) Icons.Outlined.ChatBubble else Icons.Outlined.ChatBubbleOutline,
                contentDescription = "Danmu",
            )
        }

        Box {
            FilledTonalIconButton(onClick = { menu = true }, modifier = Modifier.size(42.dp)) {
                Icon(Icons.Outlined.MoreVert, contentDescription = "More")
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                DropdownMenuItem(
                    text = { Text("播放设置") },
                    onClick = { menu = false; onOpenMenuPlaybackSettings() },
                    leadingIcon = { Icon(Icons.Outlined.Settings, contentDescription = null) },
                )
                DropdownMenuItem(
                    text = { Text("弹幕设置") },
                    onClick = { menu = false; onOpenMenuDanmuSettings() },
                    leadingIcon = { Icon(Icons.Outlined.Tune, contentDescription = null) },
                )
                DropdownMenuItem(
                    text = { Text(if (muted) "取消静音" else "静音") },
                    onClick = { menu = false; onOpenMenuToggleMute() },
                    leadingIcon = {
                        Icon(
                            imageVector = if (muted) Icons.AutoMirrored.Outlined.VolumeOff else Icons.AutoMirrored.Outlined.VolumeUp,
                            contentDescription = null,
                        )
                    },
                )
                DropdownMenuItem(
                    text = { Text("旋转") },
                    onClick = { menu = false; onOpenMenuRotate() },
                    leadingIcon = { Icon(Icons.Outlined.ScreenRotation, contentDescription = null) },
                )
                DropdownMenuItem(
                    text = { Text("画中画") },
                    onClick = { menu = false; onOpenMenuPip() },
                    leadingIcon = { Icon(Icons.Outlined.PictureInPictureAlt, contentDescription = null) },
                )
            }
        }
    }
}
