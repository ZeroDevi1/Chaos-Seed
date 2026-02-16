package com.zerodevi1.chaos_seed.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import coil.compose.AsyncImage

@Composable
fun NetworkImage(
    url: String?,
    modifier: Modifier = Modifier,
) {
    val u = (url ?: "").trim()
    if (u.isEmpty()) {
        Box(modifier = modifier.background(Color.Black.copy(alpha = 0.08f)))
        return
    }
    AsyncImage(
        model = u,
        contentDescription = null,
        modifier = modifier.fillMaxSize(),
        contentScale = ContentScale.Crop,
    )
}

