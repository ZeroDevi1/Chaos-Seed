package com.zerodevi1.chaos_seed.ui.screens

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NoticesScreen(onBack: () -> Unit) {
    val ctx = LocalContext.current
    val (text, setText) = remember { mutableStateOf("Loading...") }
    val scroll = rememberScrollState()

    LaunchedEffect(Unit) {
        val loaded = runCatching {
            ctx.assets.open("third_party_notices.txt").bufferedReader().use { it.readText() }
        }.getOrElse { e ->
            "Failed to load notices: ${e.message}"
        }
        setText(loaded)
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("开源许可与第三方声明") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(painterResource(android.R.drawable.ic_menu_close_clear_cancel), contentDescription = "Back")
                    }
                },
            )
        },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
                .verticalScroll(scroll),
        ) {
            Text(text)
        }
    }
}

