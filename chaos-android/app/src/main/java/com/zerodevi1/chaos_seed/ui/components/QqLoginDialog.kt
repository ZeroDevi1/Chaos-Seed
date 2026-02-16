package com.zerodevi1.chaos_seed.ui.components

import android.graphics.BitmapFactory
import android.util.Base64
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.unit.dp
import com.zerodevi1.chaos_seed.core.backend.LocalBackend
import com.zerodevi1.chaos_seed.core.model.QqMusicCookie
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json

private enum class QqLoginType(val id: String, val label: String) {
    Qq("qq", "QQ"),
    Wechat("wechat", "微信"),
}

@Composable
fun QqLoginDialog(
    onDismiss: () -> Unit,
    onCookie: (cookieJson: String) -> Unit,
) {
    val backend = LocalBackend.current
    val scope = rememberCoroutineScope()
    val json = remember {
        Json {
            ignoreUnknownKeys = true
            isLenient = true
        }
    }

    var type by remember { mutableStateOf(QqLoginType.Qq) }
    var loading by remember { mutableStateOf(false) }
    var err by remember { mutableStateOf<String?>(null) }
    var sessionId by remember { mutableStateOf<String?>(null) }
    var mime by remember { mutableStateOf<String?>(null) }
    var base64 by remember { mutableStateOf<String?>(null) }
    var state by remember { mutableStateOf("init") }

    fun createQr() {
        if (loading) return
        scope.launch {
            loading = true
            err = null
            sessionId = null
            mime = null
            base64 = null
            state = "init"
            try {
                val qr = backend.qqLoginQrCreate(type.id)
                sessionId = qr.sessionId
                mime = qr.mime
                base64 = qr.base64
                state = "scan"
            } catch (e: Exception) {
                err = e.toString()
            } finally {
                loading = false
            }
        }
    }

    LaunchedEffect(type) { createQr() }

    LaunchedEffect(sessionId) {
        val sid = sessionId?.trim().orEmpty()
        if (sid.isEmpty()) return@LaunchedEffect
        while (true) {
            delay(2000)
            runCatching {
                val r = backend.qqLoginQrPoll(sid)
                state = r.state
                val c = r.cookie
                if (c != null && r.state.lowercase() == "done") {
                    onCookie(json.encodeToString(QqMusicCookie.serializer(), c))
                    return@LaunchedEffect
                }
            }.onFailure { err = it.toString() }
        }
    }

    val bytes = remember(base64) {
        val b = base64?.trim().orEmpty()
        if (b.isEmpty()) null else runCatching { Base64.decode(b, Base64.DEFAULT) }.getOrNull()
    }
    val bmp = remember(bytes) {
        bytes?.let { runCatching { BitmapFactory.decodeByteArray(it, 0, it.size) }.getOrNull() }
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("QQ 音乐扫码登录") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Row {
                    Text("登录方式：")
                    Spacer(Modifier.width(8.dp))
                    var typeMenu by remember { mutableStateOf(false) }
                    TextButton(onClick = { typeMenu = true }, enabled = !loading) { Text(type.label) }
                    DropdownMenu(expanded = typeMenu, onDismissRequest = { typeMenu = false }) {
                        QqLoginType.entries.forEach { t ->
                            DropdownMenuItem(
                                text = { Text(t.label) },
                                onClick = {
                                    typeMenu = false
                                    type = t
                                },
                            )
                        }
                    }
                    Spacer(Modifier.weight(1f))
                    TextButton(onClick = { createQr() }, enabled = !loading) { Text("刷新二维码") }
                }
                if (err != null) ErrorCard(message = err!!, onDismiss = { err = null })
                if (loading) LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                if (bmp != null) {
                    Image(
                        bitmap = bmp.asImageBitmap(),
                        contentDescription = null,
                        modifier = Modifier.size(240.dp),
                    )
                } else {
                    Text("二维码生成中...")
                    Spacer(Modifier.height(240.dp))
                }
                Text(stateLabel(state))
                if (!mime.isNullOrBlank()) Text("格式：${mime!!.trim()}")
            }
        },
        confirmButton = { },
        dismissButton = { TextButton(onClick = onDismiss) { Text("取消") } },
    )
}

private fun stateLabel(raw: String): String {
    return when (raw.lowercase()) {
        "scan" -> "等待扫码"
        "confirm" -> "请在手机确认登录"
        "done" -> "登录成功"
        "timeout" -> "二维码已过期"
        "refuse" -> "已拒绝登录"
        else -> "状态：$raw"
    }
}

