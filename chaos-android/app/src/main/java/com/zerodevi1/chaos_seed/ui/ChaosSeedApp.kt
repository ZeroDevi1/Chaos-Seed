package com.zerodevi1.chaos_seed.ui

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Category
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.LibraryMusic
import androidx.compose.material.icons.outlined.LiveTv
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.zerodevi1.chaos_seed.core.backend.BackendHolder
import com.zerodevi1.chaos_seed.core.backend.LocalBackend
import com.zerodevi1.chaos_seed.player.PlayerActivity
import com.zerodevi1.chaos_seed.ui.screens.CategoriesScreen
import com.zerodevi1.chaos_seed.ui.screens.CategoryRoomsScreen
import com.zerodevi1.chaos_seed.ui.screens.HomeScreen
import com.zerodevi1.chaos_seed.ui.screens.LiveDecodeScreen
import com.zerodevi1.chaos_seed.ui.screens.MusicScreen
import com.zerodevi1.chaos_seed.ui.screens.NoticesScreen
import com.zerodevi1.chaos_seed.ui.screens.SettingsScreen

private enum class TabDest(
    val key: String,
    val label: String,
) {
    Home("home", "主页"),
    Categories("categories", "分类"),
    Live("live", "直播"),
    Music("music", "歌曲"),
    Settings("settings", "设置"),
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChaosSeedApp() {
    val appCtx = LocalContext.current.applicationContext
    val backend = remember { BackendHolder.get(appCtx) }

    CompositionLocalProvider(LocalBackend provides backend) {
        val holder = rememberSaveableStateHolder()
        var selectedTabIdx by rememberSaveable { mutableIntStateOf(0) }
        val tabs = TabDest.entries

        val navMap = remember { mutableStateMapOf<String, NavHostController>() }
        var pendingLiveRoute by rememberSaveable { mutableStateOf<String?>(null) }

        fun openLiveDecode(input: String?, autoDecode: Boolean) {
            selectedTabIdx = TabDest.Live.ordinal
            val route = buildString {
                append("live_decode")
                if (!input.isNullOrBlank() || autoDecode) {
                    append("?")
                    if (!input.isNullOrBlank()) append("input=").append(java.net.URLEncoder.encode(input, "UTF-8"))
                    if (autoDecode) {
                        if (!input.isNullOrBlank()) append("&")
                        append("auto=1")
                    }
                }
            }
            pendingLiveRoute = route

            // If Live tab is already composed, navigate immediately. Otherwise it will be consumed when
            // the Live tab's NavHostController is created.
            navMap[TabDest.Live.key]?.let { nav ->
                nav.navigate(route)
                pendingLiveRoute = null
            }
        }

        val wDp = LocalConfiguration.current.screenWidthDp
        val wide = wDp >= 840

        Scaffold(
            contentWindowInsets = WindowInsets.safeDrawing,
            bottomBar = {
                if (wide) return@Scaffold
                NavigationBar {
                    tabs.forEachIndexed { idx, t ->
                        NavigationBarItem(
                            selected = selectedTabIdx == idx,
                            onClick = { selectedTabIdx = idx },
                            icon = { TabIcon(t) },
                            label = { Text(t.label) },
                        )
                    }
                }
            },
        ) { padding ->
            Row(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
            ) {
                if (wide) {
                    NavigationRail(modifier = Modifier.padding(top = 8.dp)) {
                        tabs.forEachIndexed { idx, t ->
                            NavigationRailItem(
                                selected = selectedTabIdx == idx,
                                onClick = { selectedTabIdx = idx },
                                icon = { TabIcon(t) },
                                label = { Text(t.label) },
                            )
                        }
                    }
                }

                val tab = tabs[selectedTabIdx]
                holder.SaveableStateProvider(tab.key) {
                    val nav = rememberNavController()
                    LaunchedEffect(nav) { navMap[tab.key] = nav }
                    TabNavHost(
                        tab = tab,
                        nav = nav,
                        onOpenLiveDecode = ::openLiveDecode,
                        pendingLiveRoute = pendingLiveRoute,
                        consumePendingLiveRoute = { pendingLiveRoute = null },
                    )
                }
            }
        }
    }
}

@Composable
private fun TabIcon(t: TabDest) {
    when (t) {
        TabDest.Home -> androidx.compose.material3.Icon(Icons.Outlined.Home, contentDescription = t.label)
        TabDest.Categories -> androidx.compose.material3.Icon(Icons.Outlined.Category, contentDescription = t.label)
        TabDest.Live -> androidx.compose.material3.Icon(Icons.Outlined.LiveTv, contentDescription = t.label)
        TabDest.Music -> androidx.compose.material3.Icon(Icons.Outlined.LibraryMusic, contentDescription = t.label)
        TabDest.Settings -> androidx.compose.material3.Icon(Icons.Outlined.Settings, contentDescription = t.label)
    }
}

@Composable
private fun TabNavHost(
    tab: TabDest,
    nav: NavHostController,
    onOpenLiveDecode: (input: String?, autoDecode: Boolean) -> Unit,
    pendingLiveRoute: String?,
    consumePendingLiveRoute: () -> Unit,
) {
    when (tab) {
        TabDest.Home -> {
            NavHost(navController = nav, startDestination = "home") {
                composable("home") { HomeScreen(onOpenLiveDecode = onOpenLiveDecode) }
            }
        }
        TabDest.Categories -> {
            NavHost(navController = nav, startDestination = "categories") {
                composable("categories") {
                    CategoriesScreen(onOpenCategoryRooms = { site, parentName, parentId, subId, subName ->
                        val route = "category_rooms?site=$site&parentName=${enc(parentName)}&parentId=${enc(parentId)}&subId=${enc(subId)}&subName=${enc(subName)}"
                        nav.navigate(route)
                    })
                }
                composable("category_rooms?site={site}&parentName={parentName}&parentId={parentId}&subId={subId}&subName={subName}") { back ->
                    val site = back.arguments?.getString("site").orEmpty()
                    val parentName = dec(back.arguments?.getString("parentName"))
                    val parentId = dec(back.arguments?.getString("parentId"))
                    val subId = dec(back.arguments?.getString("subId"))
                    val subName = dec(back.arguments?.getString("subName"))
                    CategoryRoomsScreen(
                        site = site,
                        parentName = parentName ?: "",
                        parentId = parentId ?: "",
                        subId = subId ?: "",
                        subName = subName ?: "",
                        onBack = { nav.popBackStack() },
                        onOpenLiveDecode = onOpenLiveDecode,
                    )
                }
            }
        }
        TabDest.Live -> {
            NavHost(navController = nav, startDestination = "live_decode") {
                composable("live_decode") {
                    val ctx = LocalContext.current
                    LiveDecodeScreen(
                        initialInput = null,
                        autoDecode = false,
                        onOpenPlayer = { input, variantId ->
                            ctx.startActivity(PlayerActivity.intentForLive(ctx, input, variantId))
                        },
                    )
                }
                composable("live_decode?input={input}&auto={auto}") { back ->
                    val input = dec(back.arguments?.getString("input"))
                    val auto = back.arguments?.getString("auto") == "1"
                    val ctx = LocalContext.current
                    LiveDecodeScreen(
                        initialInput = input,
                        autoDecode = auto,
                        onOpenPlayer = { raw, variantId ->
                            ctx.startActivity(PlayerActivity.intentForLive(ctx, raw, variantId))
                        },
                    )
                }
            }

            LaunchedEffect(pendingLiveRoute) {
                val r = pendingLiveRoute?.trim().orEmpty()
                if (r.isEmpty()) return@LaunchedEffect
                // Avoid pushing the same destination repeatedly if recomposed.
                if (nav.currentDestination?.route == r) {
                    consumePendingLiveRoute()
                    return@LaunchedEffect
                }
                nav.navigate(r)
                consumePendingLiveRoute()
            }
        }
        TabDest.Music -> {
            NavHost(navController = nav, startDestination = "music") {
                composable("music") { MusicScreen() }
            }
        }
        TabDest.Settings -> {
            NavHost(navController = nav, startDestination = "settings") {
                composable("settings") { SettingsScreen(onOpenNotices = { nav.navigate("notices") }) }
                composable("notices") { NoticesScreen(onBack = { nav.popBackStack() }) }
            }
        }
    }
}

private fun enc(s: String?): String = java.net.URLEncoder.encode(s ?: "", "UTF-8")
private fun dec(s: String?): String? = s?.let { java.net.URLDecoder.decode(it, "UTF-8") }
