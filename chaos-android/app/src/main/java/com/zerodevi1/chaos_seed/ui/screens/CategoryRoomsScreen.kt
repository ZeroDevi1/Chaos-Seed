package com.zerodevi1.chaos_seed.ui.screens

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.material.ExperimentalMaterialApi
import androidx.compose.material.pullrefresh.PullRefreshIndicator
import androidx.compose.material.pullrefresh.pullRefresh
import androidx.compose.material.pullrefresh.rememberPullRefreshState
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.unit.dp
import com.zerodevi1.chaos_seed.core.backend.LocalBackend
import com.zerodevi1.chaos_seed.core.model.LiveDirRoomCard
import com.zerodevi1.chaos_seed.ui.components.ErrorCard
import com.zerodevi1.chaos_seed.ui.components.RoomCard
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterialApi::class)
@Composable
fun CategoryRoomsScreen(
    site: String,
    parentName: String,
    parentId: String,
    subId: String,
    subName: String,
    onBack: () -> Unit,
    onOpenLiveDecode: (input: String?, autoDecode: Boolean) -> Unit,
) {
    val backend = LocalBackend.current
    val scope = rememberCoroutineScope()

    var loading by remember { mutableStateOf(false) }
    var loadingMore by remember { mutableStateOf(false) }
    var err by remember { mutableStateOf<String?>(null) }
    var items by remember { mutableStateOf<List<LiveDirRoomCard>>(emptyList()) }
    var page by remember { mutableIntStateOf(1) }
    var hasMore by remember { mutableStateOf(true) }

    fun load(reset: Boolean) {
        if (loading || loadingMore) return
        if (!reset && !hasMore) return
        scope.launch {
            if (reset) loading = true else loadingMore = true
            err = null
            try {
                val p = if (reset) 1 else page
                val res = backend.categoryRooms(site, parentId, subId, p)
                items = if (reset) res.items else items + res.items
                hasMore = res.hasMore && res.items.isNotEmpty()
                page = if (reset) 2 else (if (hasMore) page + 1 else page)
            } catch (e: Exception) {
                err = e.toString()
            } finally {
                loading = false
                loadingMore = false
            }
        }
    }

    LaunchedEffect(site, parentId, subId) { load(reset = true) }

    val wDp = LocalConfiguration.current.screenWidthDp
    val crossAxisCount = remember(wDp) { maxOf(2, (wDp / 200f).toInt()) }
    val gridState = rememberLazyGridState()

    LaunchedEffect(gridState) {
        while (true) {
            val last = gridState.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0
            val total = gridState.layoutInfo.totalItemsCount
            if (total > 0 && last >= total - 8) {
                load(reset = false)
            }
            kotlinx.coroutines.delay(350)
        }
    }

    val refreshing = loading && items.isEmpty()
    val pullState = rememberPullRefreshState(refreshing = refreshing, onRefresh = { load(reset = true) })

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(text = subName.ifBlank { "分类" }) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            painter = androidx.compose.ui.res.painterResource(android.R.drawable.ic_media_previous),
                            contentDescription = "Back",
                        )
                    }
                },
            )
        },
    ) { inner ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(inner)
                .pullRefresh(pullState),
        ) {
            LazyVerticalGrid(
                columns = GridCells.Fixed(crossAxisCount),
                state = gridState,
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.spacedBy(12.dp),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                contentPadding = PaddingValues(12.dp),
            ) {
                item(span = { GridItemSpan(maxLineSpan) }) {
                    Column {
                        Text(parentName, style = MaterialTheme.typography.bodySmall)
                        if (err != null) {
                            ErrorCard(message = err!!, onDismiss = { err = null }, modifier = Modifier.padding(top = 6.dp))
                        }
                        if (loading && items.isNotEmpty()) {
                            LinearProgressIndicator(modifier = Modifier.padding(top = 6.dp))
                        }
                        Box(modifier = Modifier.padding(bottom = 8.dp))
                    }
                }

                items(count = items.size, key = { items[it].roomId + ":" + it }) { i ->
                    val r = items[i]
                    RoomCard(room = r, onClick = { onOpenLiveDecode(r.input, true) })
                }

                item(span = { GridItemSpan(maxLineSpan) }) {
                    when {
                        loadingMore -> Text("加载中...", modifier = Modifier.padding(vertical = 12.dp))
                        !hasMore && items.isNotEmpty() -> Text(
                            "没有更多了",
                            style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.padding(vertical = 12.dp),
                        )
                    }
                }
            }

            PullRefreshIndicator(
                refreshing = refreshing,
                state = pullState,
                modifier = Modifier.align(Alignment.TopCenter),
            )
        }
    }
}
