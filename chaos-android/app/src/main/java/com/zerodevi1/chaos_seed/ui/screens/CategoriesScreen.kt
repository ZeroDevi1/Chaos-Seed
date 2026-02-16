package com.zerodevi1.chaos_seed.ui.screens

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material.ExperimentalMaterialApi
import androidx.compose.material.pullrefresh.PullRefreshIndicator
import androidx.compose.material.pullrefresh.pullRefresh
import androidx.compose.material.pullrefresh.rememberPullRefreshState
import androidx.compose.material3.ExperimentalMaterial3Api
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
import com.zerodevi1.chaos_seed.core.model.LiveDirCategory
import com.zerodevi1.chaos_seed.ui.components.CategoryCard
import com.zerodevi1.chaos_seed.ui.components.ErrorCard
import com.zerodevi1.chaos_seed.ui.components.LiveSiteTabs
import com.zerodevi1.chaos_seed.ui.components.LiveSites
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CategoriesScreen(
    onOpenCategoryRooms: (site: String, parentName: String, parentId: String, subId: String, subName: String) -> Unit,
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
            CategoriesSiteTab(
                siteKey = site.key,
                onOpenCategoryRooms = onOpenCategoryRooms,
            )
        }
    }
}

@OptIn(ExperimentalMaterialApi::class)
@Composable
private fun CategoriesSiteTab(
    siteKey: String,
    onOpenCategoryRooms: (site: String, parentName: String, parentId: String, subId: String, subName: String) -> Unit,
) {
    val backend = LocalBackend.current
    val scope = rememberCoroutineScope()

    var loading by remember(siteKey) { mutableStateOf(false) }
    var err by remember(siteKey) { mutableStateOf<String?>(null) }
    var cats by remember(siteKey) { mutableStateOf<List<LiveDirCategory>>(emptyList()) }

    fun load() {
        if (loading) return
        scope.launch {
            loading = true
            err = null
            cats = emptyList()
            try {
                cats = backend.categories(siteKey)
            } catch (e: Exception) {
                err = e.toString()
            } finally {
                loading = false
            }
        }
    }

    LaunchedEffect(siteKey) { load() }

    val wDp = LocalConfiguration.current.screenWidthDp
    val crossAxisCount = remember(wDp) { maxOf(2, (wDp / 200f).toInt()) }
    val gridState = rememberLazyGridState()
    val pullState = rememberPullRefreshState(refreshing = loading && cats.isEmpty(), onRefresh = { load() })

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
            if (loading && cats.isNotEmpty()) {
                item(span = { GridItemSpan(maxLineSpan) }) {
                    LinearProgressIndicator(modifier = Modifier.padding(bottom = 8.dp))
                }
            }

            cats.forEach { parent ->
                item(span = { GridItemSpan(maxLineSpan) }) {
                    Text(
                        text = parent.name,
                        style = MaterialTheme.typography.titleSmall,
                        modifier = Modifier.padding(top = 4.dp, bottom = 8.dp),
                    )
                }

                items(count = parent.children.size, key = { i -> parent.id + ":" + parent.children[i].id }) { i ->
                    val sub = parent.children[i]
                    CategoryCard(
                        parent = parent,
                        sub = sub,
                        onClick = {
                            onOpenCategoryRooms(siteKey, parent.name, parent.id, sub.id, sub.name)
                        },
                    )
                }

                item(span = { GridItemSpan(maxLineSpan) }) {
                    Box(modifier = Modifier.padding(bottom = 12.dp))
                }
            }
        }

        PullRefreshIndicator(
            refreshing = loading && cats.isEmpty(),
            state = pullState,
            modifier = Modifier.align(Alignment.TopCenter),
        )
    }
}
