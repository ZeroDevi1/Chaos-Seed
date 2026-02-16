package com.zerodevi1.chaos_seed.ui.screens

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material.icons.outlined.PlayArrow
import androidx.compose.material.icons.outlined.PlayCircleOutline
import androidx.compose.material.icons.outlined.UnfoldLess
import androidx.compose.material.icons.outlined.UnfoldMore
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.zerodevi1.chaos_seed.core.backend.LocalBackend
import com.zerodevi1.chaos_seed.core.model.LivestreamDecodeManifestResult
import com.zerodevi1.chaos_seed.core.model.LivestreamVariant
import com.zerodevi1.chaos_seed.ui.components.ErrorCard
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LiveDecodeScreen(
    initialInput: String?,
    autoDecode: Boolean,
    onOpenPlayer: (input: String, variantId: String) -> Unit,
) {
    val backend = LocalBackend.current
    val scope = rememberCoroutineScope()

    var input by remember { mutableStateOf((initialInput ?: "").trim()) }
    var expanded by remember { mutableStateOf(true) }

    var loading by remember { mutableStateOf(false) }
    var err by remember { mutableStateOf<String?>(null) }
    var man by remember { mutableStateOf<LivestreamDecodeManifestResult?>(null) }

    fun decode() {
        val s = input.trim()
        if (s.isEmpty()) return
        if (loading) return
        scope.launch {
            loading = true
            err = null
            man = null
            try {
                man = backend.decodeManifest(s)
            } catch (e: Exception) {
                err = e.toString()
            } finally {
                loading = false
            }
        }
    }

    LaunchedEffect(autoDecode, initialInput) {
        if (autoDecode && input.trim().isNotEmpty()) decode()
    }

    Scaffold(
        topBar = { TopAppBar(title = { Text("链接解析") }) },
    ) { inner ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(inner)
                .padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            OutlinedCard {
                Column {
                    ListItem(
                        headlineContent = { Text("直播间解析") },
                        supportingContent = { Text("输入或粘贴哔哩哔哩/虎牙/斗鱼直播链接或 roomId") },
                        trailingContent = {
                            IconButton(onClick = { expanded = !expanded }) {
                                Icon(
                                    imageVector = if (expanded) Icons.Outlined.UnfoldLess else Icons.Outlined.UnfoldMore,
                                    contentDescription = null,
                                )
                            }
                        },
                        modifier = Modifier.clickable { expanded = !expanded },
                    )
                    AnimatedVisibility(visible = expanded) {
                        Column(modifier = Modifier.padding(start = 12.dp, end = 12.dp, bottom = 12.dp)) {
                            OutlinedTextField(
                                value = input,
                                onValueChange = { input = it },
                                minLines = 2,
                                maxLines = 3,
                                label = { Text("直播链接/roomId") },
                                modifier = Modifier.fillMaxWidth(),
                            )
                            TextButton(
                                onClick = { decode() },
                                enabled = !loading,
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(top = 8.dp),
                            ) {
                                Icon(Icons.Outlined.PlayCircleOutline, contentDescription = null)
                                Text("解析", modifier = Modifier.padding(start = 8.dp))
                            }
                        }
                    }
                }
            }

            if (err != null) {
                ErrorCard(message = err!!, onDismiss = { err = null })
            }
            if (loading) LinearProgressIndicator(modifier = Modifier.fillMaxWidth())

            val m = man
            if (m != null) {
                Card {
                    ListItem(
                        headlineContent = { Text(if (m.info.title.isBlank()) "已解析" else m.info.title) },
                        supportingContent = { Text("${m.site}:${m.roomId}  清晰度选项=${m.variants.size}") },
                    )
                }
                val variants = remember(m) { m.variants.sortedByDescending { it.quality } }
                variants.forEach { v ->
                    VariantCard(v = v, enabled = !loading) {
                        onOpenPlayer(input.trim(), v.id)
                    }
                }
            } else {
                Card {
                    ListItem(
                        leadingContent = { Icon(Icons.Outlined.Info, contentDescription = null) },
                        headlineContent = { Text("提示") },
                        supportingContent = { Text("解析后会显示清晰度列表；点击即可进入播放页。") },
                    )
                }
            }
        }
    }
}

@Composable
private fun VariantCard(
    v: LivestreamVariant,
    enabled: Boolean,
    onClick: () -> Unit,
) {
    Card {
        ListItem(
            headlineContent = { Text(v.label) },
            supportingContent = { Text("清晰度：${v.quality}") },
            trailingContent = { Icon(Icons.Outlined.PlayArrow, contentDescription = null) },
            modifier = Modifier.clickable(enabled = enabled, onClick = onClick),
        )
    }
}

