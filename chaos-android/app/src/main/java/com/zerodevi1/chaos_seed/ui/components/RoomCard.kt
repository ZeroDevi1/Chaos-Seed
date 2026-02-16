package com.zerodevi1.chaos_seed.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.LocalFireDepartment
import androidx.compose.material3.Card
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.SubcomposeLayout
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.zerodevi1.chaos_seed.core.model.LiveDirRoomCard

@Composable
fun RoomCard(
    room: LiveDirRoomCard,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Card(modifier = modifier.clickable(onClick = onClick)) {
        // Use a layout pass to decide whether we are "compact" (grid cell height too small).
        SubcomposeLayout { constraints ->
            val maxH = constraints.maxHeight
            val compact = maxH in 1..179
            val titleLines = if (compact) 1 else 2
            val showUser = !compact
            val pad = if (compact) PaddingValues(start = 12.dp, top = 8.dp, end = 12.dp, bottom = 10.dp)
            else PaddingValues(start = 12.dp, top = 10.dp, end = 12.dp, bottom = 12.dp)

            val p = subcompose("content") {
                Column {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .aspectRatio(16f / 9f),
                    ) {
                        NetworkImage(url = room.cover, modifier = Modifier.fillMaxSize())

                        val online = formatOnlineCount(room.online)
                        if (online != null) {
                            Box(
                                modifier = Modifier
                                    .align(Alignment.BottomCenter)
                                    .fillMaxWidth()
                                    .background(
                                        brush = Brush.verticalGradient(
                                            colors = listOf(
                                                Color.Black.copy(alpha = 0.75f),
                                                Color.Transparent,
                                            ),
                                        ),
                                    )
                                    .padding(start = 6.dp, top = 10.dp, end = 6.dp, bottom = 6.dp),
                            ) {
                                Row(
                                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    modifier = Modifier.align(Alignment.CenterEnd),
                                ) {
                                    Icon(
                                        imageVector = Icons.Outlined.LocalFireDepartment,
                                        contentDescription = null,
                                        tint = Color.White,
                                    )
                                    Text(
                                        text = online,
                                        color = Color.White,
                                        style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.SemiBold),
                                    )
                                }
                            }
                        }
                    }

                    Column(modifier = Modifier.padding(pad)) {
                        Text(
                            text = room.title,
                            maxLines = titleLines,
                            overflow = TextOverflow.Ellipsis,
                            style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold),
                        )
                        if (showUser) {
                            Text(
                                text = room.userName.orEmpty(),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                                style = MaterialTheme.typography.bodySmall.copy(color = MaterialTheme.colorScheme.onSurfaceVariant),
                                modifier = Modifier.padding(top = 6.dp),
                            )
                        }
                    }
                }
            }.map { it.measure(constraints) }

            val w = constraints.maxWidth
            val h = p.maxOfOrNull { it.height } ?: 0
            layout(w, h) { p.forEach { it.placeRelative(0, 0) } }
        }
    }
}
