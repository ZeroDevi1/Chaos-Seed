package com.zerodevi1.chaos_seed.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ListItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.zerodevi1.chaos_seed.player.PlayerEngineType
import com.zerodevi1.chaos_seed.player.PlayerSessionRegistry
import com.zerodevi1.chaos_seed.settings.AppThemeMode
import com.zerodevi1.chaos_seed.settings.SettingsViewModel
import com.zerodevi1.chaos_seed.ui.components.QqLoginDialog

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    onOpenNotices: () -> Unit,
    vm: SettingsViewModel = viewModel(),
) {
    val s by vm.state.collectAsState()

    var engineMenu by remember { mutableStateOf(false) }
    var themeMenu by remember { mutableStateOf(false) }
    var showQqLogin by remember { mutableStateOf(false) }
    var confirmEngine: PlayerEngineType? by remember { mutableStateOf(null) }

    fun requestEngineChange(next: PlayerEngineType) {
        if (PlayerSessionRegistry.isActive.value) confirmEngine = next
        else vm.setPlayerEngine(next)
    }

    if (confirmEngine != null) {
        val next = confirmEngine!!
        AlertDialog(
            onDismissRequest = { confirmEngine = null },
            title = { Text("切换播放引擎") },
            text = { Text("将切换到 ${next.label} 并重新打开当前线路。") },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirmEngine = null
                        vm.setPlayerEngine(next)
                        PlayerSessionRegistry.controller?.requestSwitchEngine(next)
                    },
                ) { Text("切换") }
            },
            dismissButton = { TextButton(onClick = { confirmEngine = null }) { Text("取消") } },
        )
    }

    Scaffold(
        topBar = { TopAppBar(title = { Text("设置") }) },
    ) { inner ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(inner)
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            OutlinedCard {
                ListItem(
                    headlineContent = { Text("主题模式") },
                    supportingContent = { Text("跟随系统 / 强制深色 / 强制浅色。Android 12+ 支持 Material You 动态取色。") },
                    trailingContent = {
                        Column {
                            OutlinedButton(onClick = { themeMenu = true }) {
                                Text(
                                    when (s.themeMode) {
                                        AppThemeMode.System -> "system"
                                        AppThemeMode.Dark -> "dark"
                                        AppThemeMode.Light -> "light"
                                    },
                                )
                            }
                            DropdownMenu(expanded = themeMenu, onDismissRequest = { themeMenu = false }) {
                                DropdownMenuItem(
                                    text = { Text("system") },
                                    onClick = { themeMenu = false; vm.setThemeMode(AppThemeMode.System) },
                                )
                                DropdownMenuItem(
                                    text = { Text("dark") },
                                    onClick = { themeMenu = false; vm.setThemeMode(AppThemeMode.Dark) },
                                )
                                DropdownMenuItem(
                                    text = { Text("light") },
                                    onClick = { themeMenu = false; vm.setThemeMode(AppThemeMode.Light) },
                                )
                            }
                        }
                    },
                )
            }

            OutlinedCard {
                ListItem(
                    headlineContent = { Text("播放引擎") },
                    supportingContent = { Text("EXO / MPV 可切换。MPV 体积更大；若 MPV 初始化失败会自动回退 EXO。") },
                    trailingContent = {
                        Column {
                            OutlinedButton(onClick = { engineMenu = true }) { Text(s.playerEngine.label) }
                            DropdownMenu(expanded = engineMenu, onDismissRequest = { engineMenu = false }) {
                                PlayerEngineType.entries.forEach { t ->
                                    DropdownMenuItem(
                                        text = { Text(t.label) },
                                        onClick = {
                                            engineMenu = false
                                            requestEngineChange(t)
                                        },
                                    )
                                }
                            }
                        }
                    },
                )
            }

            OutlinedCard {
                ListItem(
                    headlineContent = { Text("PiP 时隐藏弹幕") },
                    supportingContent = { Text("进入画中画时临时关闭弹幕并清空，退出后恢复。") },
                    trailingContent = {
                        Switch(
                            checked = s.pipHideDanmaku,
                            onCheckedChange = { vm.setPipHideDanmaku(it) },
                        )
                    },
                )
            }

            OutlinedCard {
                ListItem(
                    headlineContent = { Text("弹幕默认开启") },
                    supportingContent = { Text("直播播放器内的弹幕显示开关默认值（可在播放页内随时切换）。") },
                    trailingContent = {
                        Switch(
                            checked = s.danmakuEnabled,
                            onCheckedChange = { vm.setDanmakuEnabled(it) },
                        )
                    },
                )
            }

            OutlinedCard {
                ListItem(
                    headlineContent = { Text("QQ 音乐登录") },
                    supportingContent = {
                        Text(if (s.qqMusicCookieJson.isNullOrBlank()) "未登录" else "已登录（Cookie 已缓存）")
                    },
                    trailingContent = {
                        Row {
                            TextButton(onClick = { showQqLogin = true }) { Text("重新登录") }
                            Spacer(Modifier.width(6.dp))
                            TextButton(
                                onClick = { vm.setQqMusicCookieJson(null) },
                                enabled = !s.qqMusicCookieJson.isNullOrBlank(),
                            ) { Text("退出") }
                        }
                    },
                )
            }

            Card {
                Column(modifier = Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text("音乐下载", style = androidx.compose.material3.MaterialTheme.typography.titleSmall)
                    Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedTextField(
                            value = s.musicDownloadConcurrency.toString(),
                            onValueChange = { raw -> raw.toIntOrNull()?.let { vm.setMusicDownloadConcurrency(it) } },
                            label = { Text("并发") },
                            singleLine = true,
                            modifier = Modifier.weight(1f),
                        )
                        OutlinedTextField(
                            value = s.musicDownloadRetries.toString(),
                            onValueChange = { raw -> raw.toIntOrNull()?.let { vm.setMusicDownloadRetries(it) } },
                            label = { Text("重试") },
                            singleLine = true,
                            modifier = Modifier.weight(1f),
                        )
                    }
                    OutlinedTextField(
                        value = s.musicPathTemplate,
                        onValueChange = { vm.setMusicPathTemplate(it) },
                        label = { Text("路径模板（可留空）") },
                        supportingText = { Text("示例：{artist}/{title}（Rust 侧模板解析保持一致）") },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                    )
                }
            }

            OutlinedCard {
                ListItem(
                    headlineContent = { Text("开源许可与第三方声明") },
                    supportingContent = { Text("MPV/libmpv 等第三方组件的许可与说明。") },
                    trailingContent = { TextButton(onClick = onOpenNotices) { Text("查看") } },
                )
            }
        }
    }

    if (showQqLogin) {
        QqLoginDialog(
            onDismiss = { showQqLogin = false },
            onCookie = { cookieJson ->
                vm.setQqMusicCookieJson(cookieJson)
                showQqLogin = false
            },
        )
    }
}
