package com.zerodevi1.chaos_seed.ui.screens

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.ExperimentalMaterialApi
import androidx.compose.material.pullrefresh.PullRefreshIndicator
import androidx.compose.material.pullrefresh.pullRefresh
import androidx.compose.material.pullrefresh.rememberPullRefreshState
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Scaffold
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
import com.zerodevi1.chaos_seed.ui.components.LiveSiteTabs
import com.zerodevi1.chaos_seed.ui.components.LiveSites
import com.zerodevi1.chaos_seed.ui.components.RoomCard
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(
    onOpenLiveDecode: (input: String?, autoDecode: Boolean) -> Unit,
) {
    val sites = remember { LiveSites.all }
    val pager = rememberPagerState(initialPage = 0, pageCount = { sites.size })
    var tabIdx by remember { mutableIntStateOf(0) }
    val scope = rememberCoroutineScope()

    LaunchedEffect(pager.currentPage) { tabIdx = pager.currentPage }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    LiveSiteTabs(
                        sites = sites,
                        selectedIndex = tabIdx,
                        onSelect = { idx ->
                            tabIdx = idx
                            scope.launch { pager.scrollToPage(idx) }
                        },
                    )
                },
                actions = {
                    IconButton(onClick = { onOpenLiveDecode(null, false) }) {
                        Icon(Icons.Outlined.Search, contentDescription = "搜索/解析")
                    }
                },
            )
        },
    ) { inner ->
        HorizontalPager(
            state = pager,
            modifier = Modifier
                .fillMaxSize()
                .padding(inner),
        ) { page ->
            val site = sites[page]
            HomeRoomsTab(
                siteKey = site.key,
                onOpenLiveDecode = onOpenLiveDecode,
            )
        }
    }
}

@OptIn(ExperimentalMaterialApi::class)
@Composable
private fun HomeRoomsTab(
    siteKey: String,
    onOpenLiveDecode: (input: String?, autoDecode: Boolean) -> Unit,
) {
    val backend = LocalBackend.current
    val scope = rememberCoroutineScope()

    var loading by remember(siteKey) { mutableStateOf(false) }
    var loadingMore by remember(siteKey) { mutableStateOf(false) }
    var err by remember(siteKey) { mutableStateOf<String?>(null) }
    var items by remember(siteKey) { mutableStateOf<List<LiveDirRoomCard>>(emptyList()) }
    var page by remember(siteKey) { mutableIntStateOf(1) }
    var hasMore by remember(siteKey) { mutableStateOf(true) }

    fun load(reset: Boolean) {
        if (loading || loadingMore) return
        if (!reset && !hasMore) return

        scope.launch {
            if (reset) loading = true else loadingMore = true
            err = null
            try {
                val p = if (reset) 1 else page
                val res = backend.recommendRooms(siteKey, p)
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

    LaunchedEffect(siteKey) { load(reset = true) }

    val gridState = rememberLazyGridState()

    LaunchedEffect(siteKey, gridState) {
        // Infinite scroll: trigger load more when close to end (roughly aligns Flutter's -400px).
        while (true) {
            val last = gridState.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0
            val total = gridState.layoutInfo.totalItemsCount
            if (total > 0 && last >= total - 8) {
                load(reset = false)
            }
            kotlinx.coroutines.delay(350)
        }
    }

    val refreshing = loading && page <= 2 && items.isEmpty()
    val pullState = rememberPullRefreshState(refreshing = refreshing, onRefresh = { load(reset = true) })

    val wDp = LocalConfiguration.current.screenWidthDp
    val crossAxisCount = remember(wDp) { maxOf(2, (wDp / 200f).toInt()) }

    Box(
        modifier = Modifier
            .fillMaxSize()
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
            if (err != null) {
                item(span = { GridItemSpan(maxLineSpan) }) {
                    ErrorCard(message = err!!, onDismiss = { err = null })
                }
            }
            if (loading && items.isNotEmpty()) {
                item(span = { GridItemSpan(maxLineSpan) }) {
                    LinearProgressIndicator(modifier = Modifier.padding(bottom = 8.dp))
                }
            }

            items(count = items.size, key = { items[it].roomId + ":" + it }) { i ->
                val r = items[i]
                RoomCard(
                    room = r,
                    onClick = { onOpenLiveDecode(r.input, true) },
                )
            }

            item(span = { GridItemSpan(maxLineSpan) }) {
                when {
                    loadingMore -> {
                        androidx.compose.material3.Text(
                            "加载中...",
                            modifier = Modifier.padding(vertical = 12.dp),
                        )
                    }
                    !hasMore && items.isNotEmpty() -> {
                        androidx.compose.material3.Text(
                            "没有更多了",
                            modifier = Modifier.padding(vertical = 12.dp),
                        )
                    }
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
