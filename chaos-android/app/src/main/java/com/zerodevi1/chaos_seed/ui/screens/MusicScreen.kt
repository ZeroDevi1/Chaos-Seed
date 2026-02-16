package com.zerodevi1.chaos_seed.ui.screens

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ArrowDropDown
import androidx.compose.material.icons.outlined.Download
import androidx.compose.material.icons.outlined.QrCode
import androidx.compose.material.icons.outlined.Verified
import androidx.compose.material3.Card
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.zerodevi1.chaos_seed.core.backend.LocalBackend
import com.zerodevi1.chaos_seed.core.model.LyricsSearchParams
import com.zerodevi1.chaos_seed.core.model.MusicAuthState
import com.zerodevi1.chaos_seed.core.model.MusicDownloadOptions
import com.zerodevi1.chaos_seed.core.model.MusicDownloadStartParams
import com.zerodevi1.chaos_seed.core.model.MusicProviderConfig
import com.zerodevi1.chaos_seed.core.model.MusicSearchParams
import com.zerodevi1.chaos_seed.core.model.MusicService
import com.zerodevi1.chaos_seed.core.model.MusicTrack
import com.zerodevi1.chaos_seed.core.model.MusicJobState
import com.zerodevi1.chaos_seed.core.model.QqMusicCookie
import com.zerodevi1.chaos_seed.core.storage.AndroidDownloadDir
import com.zerodevi1.chaos_seed.settings.SettingsViewModel
import com.zerodevi1.chaos_seed.ui.components.QqLoginDialog
import com.zerodevi1.chaos_seed.ui.components.ErrorCard
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import java.io.File

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MusicScreen(
    vm: SettingsViewModel = viewModel(),
) {
    val backend = LocalBackend.current
    val s by vm.state.collectAsState()
    val scope = rememberCoroutineScope()
    val snackbar = remember { SnackbarHostState() }
    val json = remember {
        Json {
            ignoreUnknownKeys = true
            isLenient = true
        }
    }

    var svc by remember { mutableStateOf(MusicService.Qq) }
    var q by remember { mutableStateOf("") }
    var loading by remember { mutableStateOf(false) }
    var loadingMore by remember { mutableStateOf(false) }
    var err by remember { mutableStateOf<String?>(null) }
    var tracks by remember { mutableStateOf<List<MusicTrack>>(emptyList()) }
    var searchPage by remember { mutableStateOf(1) }
    var hasMore by remember { mutableStateOf(true) }
    var lastQuery by remember { mutableStateOf<String?>(null) }
    var lastSvc by remember { mutableStateOf<MusicService?>(null) }

    var showQqLogin by remember { mutableStateOf(false) }

    val qqLoggedIn = !s.qqMusicCookieJson.isNullOrBlank()
    val listState = rememberLazyListState()

    val pageSize = 20

    fun cfgFromSettings(): MusicProviderConfig {
        val netease = s.neteaseBaseUrls
            .split(';')
            .map { it.trim() }
            .filter { it.isNotEmpty() }
        return MusicProviderConfig(
            kugouBaseUrl = s.kugouBaseUrl,
            neteaseBaseUrls = netease,
            neteaseAnonymousCookieUrl = s.neteaseAnonymousCookieUrl.trim().ifBlank { null },
        )
    }

    fun search() {
        val kw = q.trim()
        if (kw.isEmpty() || loading || loadingMore) return
        scope.launch {
            loading = true
            err = null
            tracks = emptyList()
            searchPage = 1
            hasMore = true
            lastQuery = kw
            lastSvc = svc
            try {
                backend.musicConfigSet(cfgFromSettings())
                val items = backend.searchTracks(
                    MusicSearchParams(
                        service = svc,
                        keyword = kw,
                        page = 1,
                        pageSize = pageSize,
                    ),
                )
                tracks = items
                // Backend doesn't return hasMore; infer by page size.
                hasMore = items.size >= pageSize
                searchPage = 2
            } catch (e: Exception) {
                err = e.toString()
            } finally {
                loading = false
            }
        }
    }

    fun loadMore() {
        val kw = (lastQuery ?: q).trim()
        if (kw.isEmpty()) return
        if (!hasMore || loading || loadingMore) return
        if (lastSvc != null && lastSvc != svc) return

        scope.launch {
            loadingMore = true
            try {
                backend.musicConfigSet(cfgFromSettings())
                val items = backend.searchTracks(
                    MusicSearchParams(
                        service = svc,
                        keyword = kw,
                        page = searchPage,
                        pageSize = pageSize,
                    ),
                )

                // Append unique by (service,id) to avoid duplicates if backend jitter/repeats.
                val seen = tracks.associateBy { it.service.name + ":" + it.id }.toMutableMap()
                for (t in items) {
                    seen.putIfAbsent(t.service.name + ":" + t.id, t)
                }
                tracks = seen.values.toList()

                hasMore = items.size >= pageSize
                if (hasMore) searchPage += 1
            } catch (e: Exception) {
                snackbar.showSnackbar("加载更多失败：${e.message ?: e::class.java.simpleName}")
            } finally {
                loadingMore = false
            }
        }
    }

    // Infinite scroll paging: when reaching the end, load next page.
    LaunchedEffect(listState, tracks.size, hasMore, loading, loadingMore) {
        if (!hasMore || loading || loadingMore) return@LaunchedEffect
        val lastVisible = listState.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: return@LaunchedEffect
        if (tracks.isNotEmpty() && lastVisible >= tracks.size - 4) {
            loadMore()
        }
    }

    fun ensureLyrics(sessionId: String, track: MusicTrack) {
        // Best-effort; runs in background.
        scope.launch {
            runCatching {
                val deadline = System.currentTimeMillis() + 10 * 60_000L
                var audioPath: String? = null
                while (System.currentTimeMillis() < deadline) {
                    val st = backend.downloadStatus(sessionId)
                    if (st.done) {
                        audioPath = st.jobs.firstOrNull { it.state == MusicJobState.Done }?.path
                            ?: st.jobs.firstOrNull { !it.path.isNullOrBlank() }?.path
                        break
                    }
                    delay(1000)
                }
                val ap = audioPath?.trim().orEmpty()
                if (ap.isEmpty()) return@runCatching
                val lrcPath = ap.substringBeforeLast('.') + ".lrc"
                if (File(lrcPath).exists()) return@runCatching

                val title = track.title.trim()
                if (title.isEmpty()) return@runCatching
                val artist = track.artists.joinToString(" / ").trim().ifBlank { null }
                val items = backend.lyricsSearch(
                    LyricsSearchParams(
                        title = title,
                        album = track.album?.trim()?.ifBlank { null },
                        artist = artist,
                        durationMs = track.durationMs ?: 0L,
                        limit = 5,
                        strictMatch = false,
                        services = listOf("qq", "netease", "lrclib"),
                        timeoutMs = 8_000,
                    ),
                )
                val picked = items.filter { it.lyricsOriginal.trim().isNotEmpty() }
                if (picked.isEmpty()) return@runCatching
                val best = picked.sortedWith(
                    compareByDescending<com.zerodevi1.chaos_seed.core.model.LyricsSearchResult> { it.quality }
                        .thenByDescending { it.matchPercentage },
                ).first()

                var content = best.lyricsOriginal
                val tr = best.lyricsTranslation?.trim().orEmpty()
                if (tr.isNotEmpty()) content = content + "\n\n" + tr
                File(lrcPath).writeText(content)
            }
        }
    }

    fun download(t: MusicTrack, context: Context) {
        if (loading) return
        scope.launch {
            loading = true
            err = null
            try {
                val cfg = cfgFromSettings()
                backend.musicConfigSet(cfg)

                val outDir = AndroidDownloadDir.pickWritableDir(context)
                File(outDir).mkdirs()

                var auth = MusicAuthState()
                if (t.service == MusicService.Qq) {
                    val raw = s.qqMusicCookieJson?.trim().orEmpty()
                    if (raw.isEmpty()) error("QQ 音乐：未登录（请先扫码登录以获取 Cookie）。")
                    val cookie = json.decodeFromString(QqMusicCookie.serializer(), raw)
                    auth = MusicAuthState(qq = cookie)
                }

                val params = MusicDownloadStartParams(
                    config = cfg,
                    auth = auth,
                    target = com.zerodevi1.chaos_seed.core.model.MusicDownloadTarget.Track(track = t),
                    options = MusicDownloadOptions(
                        qualityId = t.qualities.firstOrNull()?.id ?: "standard",
                        outDir = outDir,
                        pathTemplate = s.musicPathTemplate.trim().ifBlank { null },
                        overwrite = false,
                        concurrency = s.musicDownloadConcurrency,
                        retries = s.musicDownloadRetries,
                    ),
                )

                val res = backend.downloadStart(params)
                snackbar.showSnackbar("下载开始: sessionId=${res.sessionId}")
                ensureLyrics(res.sessionId, t)
            } catch (e: Exception) {
                err = e.toString()
            } finally {
                loading = false
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("歌曲") },
                actions = {
                    if (svc == MusicService.Qq) {
                        IconButton(onClick = { showQqLogin = true }) {
                            Icon(
                                imageVector = if (qqLoggedIn) Icons.Outlined.Verified else Icons.Outlined.QrCode,
                                contentDescription = "QQ 音乐扫码登录",
                            )
                        }
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(hostState = snackbar) },
    ) { inner ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(inner)
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            if (svc == MusicService.Qq) {
                Card {
                    ListItem(
                        leadingContent = { Icon(Icons.Outlined.Verified, contentDescription = null) },
                        headlineContent = { Text(if (qqLoggedIn) "QQ 音乐：已登录（Cookie 已缓存）" else "QQ 音乐：未登录") },
                        supportingContent = { Text("下载失败时通常是未登录或 Cookie 失效，可点右上角扫码重新登录。") },
                    )
                }
            }

            Row(modifier = Modifier.fillMaxWidth()) {
                ServicePicker(current = svc, onPick = { svc = it })
                Spacer(Modifier.width(8.dp))
                OutlinedTextField(
                    value = q,
                    onValueChange = { q = it },
                    label = { Text("搜索歌曲") },
                    modifier = Modifier.weight(1f),
                    singleLine = true,
                )
                Spacer(Modifier.width(8.dp))
                FilledTonalButton(
                    onClick = { search() },
                    enabled = !loading && !loadingMore,
                ) { Text("搜索") }
            }

            if (err != null) ErrorCard(message = err!!, onDismiss = { err = null })
            if (loading) LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
            if (!loading && err == null && tracks.isNotEmpty()) {
                val more = if (hasMore) "（可继续加载）" else ""
                Text("结果：${tracks.size}$more", modifier = Modifier.padding(top = 2.dp))
            }

            val ctx = androidx.compose.ui.platform.LocalContext.current
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                state = listState,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(items = tracks, key = { it.service.name + ":" + it.id }) { t ->
                    Card {
                        ListItem(
                            headlineContent = { Text(t.title, maxLines = 1) },
                            supportingContent = { Text(t.artists.joinToString(" / "), maxLines = 1) },
                            trailingContent = {
                                IconButton(onClick = { download(t, ctx) }, enabled = !loading) {
                                    Icon(Icons.Outlined.Download, contentDescription = "下载")
                                }
                            },
                        )
                    }
                }
                item {
                    when {
                        loadingMore -> Text("加载更多中...", modifier = Modifier.padding(vertical = 12.dp))
                        hasMore && tracks.isNotEmpty() -> TextButton(
                            onClick = { loadMore() },
                            enabled = !loading && !loadingMore,
                            modifier = Modifier.fillMaxWidth(),
                        ) { Text("加载更多") }
                        !hasMore && tracks.isNotEmpty() -> Text(
                            "没有更多了",
                            modifier = Modifier.padding(vertical = 12.dp),
                        )
                    }
                }
                item { Spacer(Modifier.height(12.dp)) }
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

@Composable
private fun ServicePicker(
    current: MusicService,
    onPick: (MusicService) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    OutlinedButton(onClick = { expanded = true }) {
        Text(current.name.lowercase())
        Icon(Icons.Outlined.ArrowDropDown, contentDescription = null)
    }
    DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
        MusicService.entries.forEach { s ->
            DropdownMenuItem(
                text = { Text(s.name.lowercase()) },
                onClick = {
                    expanded = false
                    onPick(s)
                },
            )
        }
    }
}
