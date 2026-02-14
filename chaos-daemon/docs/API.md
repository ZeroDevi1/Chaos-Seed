# chaos-daemon API（NamedPipe + JSON-RPC 2.0 / 中文说明）

`chaos-daemon` 是 Windows 下的后端进程：把 `chaos-core` / `chaos-app` 的能力通过 **NamedPipe + JSON-RPC 2.0** 暴露给 WinUI 3（或任何 .NET 客户端）。

传输层使用 **LSP Content-Length framing**（每条 JSON 外包一层 `Content-Length: ...\r\n\r\n<json>`）。

本文件按 **运行时真实行为**（以 `chaos-proto` DTO + `chaos-daemon` 实现为准）整理所有可用方法、参数与结果字段形状。

## 启动、连接与鉴权

daemon 启动参数：

```txt
chaos-daemon.exe --pipe-name <PIPE_NAME> --auth-token <TOKEN>
```

连接成功后，客户端必须先调用一次 `daemon.ping` 完成鉴权：

- 在 daemon **未鉴权**前，除 `daemon.ping` 外的所有方法会返回 `Unauthorized`（`code = -32001`）。
- `authToken` 不匹配时，daemon 会返回 `Unauthorized` 并断开连接（客户端应重启/重连）。

## 通用约定

- **JSON-RPC 2.0**：每个 request 都必须带 `jsonrpc: "2.0"` 与 `id`。
- **params 必填**：当前实现要求 `params` 必须存在；即使“没有参数”的场景也请传空对象 `{}`，否则返回 `InvalidParams`（`code = -32602`）。
- **字段命名**：`camelCase`（与 `chaos-proto` 的 `#[serde(rename_all="camelCase")]` 一致）。
- **site 约定值**：`bili_live` / `huya` / `douyu`（与 WinUI3、FFI 端一致）。

### 错误码（JSON-RPC error.code）

- `-32600` `InvalidRequest`：`jsonrpc` 版本不对等。
- `-32601` `MethodNotFound`：未知方法名。
- `-32602` `InvalidParams`：缺少 `params` / 形状不匹配。
- `-32603` `InternalError`：方法内部错误（`error.message` 为文本错误）。
- `-32001` `Unauthorized`：未鉴权 / token 不匹配。

## 会话与通知模型（重要）

daemon 会把弹幕以 **服务器通知**的形式推给客户端：

- 通知方法名：`danmaku.message`
- 通知 payload：`params` = `DanmakuMessage`（见下文）

并且为了简化客户端状态管理，daemon 在“同一条连接”上做了单 session 约束：

- `live.open`：如果之前已经有一个活跃 live session，daemon 会先 **隐式 `live.close` 旧 session**（best-effort），再建立新 session。
- `danmaku.connect`：同理，会先 **隐式 `danmaku.disconnect` 旧 session**（best-effort），再建立新 session。

如果你的客户端需要同时开多个房间/会话，建议建立多条 pipe 连接（每连接一组会话）。

## 方法一览

- `daemon.ping`
- `nowPlaying.snapshot`
- `lyrics.search`
- `music.config.set`
- `music.searchTracks` / `music.searchAlbums` / `music.searchArtists`
- `music.albumTracks` / `music.artistAlbums`
- `music.trackPlayUrl`
- `music.qq.loginQrCreate` / `music.qq.loginQrPoll` / `music.qq.refreshCookie`
- `music.kugou.loginQrCreate` / `music.kugou.loginQrPoll`
- `music.download.start` / `music.download.status` / `music.download.cancel`
- `livestream.decodeManifest`
- `live.open` / `live.close`
- `danmaku.connect` / `danmaku.disconnect` / `danmaku.fetchImage`
- `liveDir.categories` / `liveDir.recommendRooms` / `liveDir.categoryRooms` / `liveDir.searchRooms`
- 通知：`danmaku.message`

## 方法详解

### `daemon.ping`（鉴权）

params:

```json
{ "authToken": "<TOKEN>" }
```

result:

```json
{ "version": "0.4.6" }
```

说明：
- 第一次调用用于鉴权；鉴权成功后可再次调用（等价“健康检查”）。

### `nowPlaying.snapshot`（系统媒体 / Windows Now Playing）

params（全部可选，但 `params` 对象本身必须存在）：

```json
{
  "includeThumbnail": false,
  "maxThumbnailBytes": 262144,
  "maxSessions": 32
}
```

默认与约束：
- `includeThumbnail`：默认 `false`。
- `maxThumbnailBytes`：默认 `262144`，并 `clamp(1, 2_500_000)`。
- `maxSessions`：默认 `32`，并 `clamp(1, 128)`。

result：`NowPlayingSnapshot`

```json
{
  "supported": true,
  "retrievedAtUnixMs": 0,
  "pickedAppId": "Spotify",
  "nowPlaying": {
    "appId": "Spotify",
    "isCurrent": true,
    "playbackStatus": "Playing",
    "title": "Song",
    "artist": "Artist",
    "albumTitle": "Album",
    "positionMs": 1234,
    "durationMs": 234567,
    "genres": [],
    "songId": "optional",
    "thumbnail": { "mime": "image/png", "base64": "..." },
    "error": null
  },
  "sessions": []
}
```

说明：
- `supported=false` 表示系统不支持/不可用（此时 `nowPlaying` 可能为 `null`）。
- `thumbnail` 只有在 `includeThumbnail=true` 且获取成功时才会出现。

### `lyrics.search`（歌词搜索）

params：

```json
{
  "title": "Hello",
  "album": "Hello",
  "artist": "Adele",
  "durationMs": 296000,
  "limit": 5,
  "strictMatch": true,
  "services": ["netease", "qq", "kugou"],
  "timeoutMs": 10000
}
```

字段说明：
- `title`：必填，空字符串会返回 `InternalError`（message = "title is empty"）。
- `album` / `artist`：可选；当 `artist` 为空时会降级为 keyword 搜索。
- `durationMs`：可选；传 `0` 会被忽略。
- `limit`：可选；daemon 内会 `clamp(1, 50)`。
- `strictMatch`：可选；是否过滤 `matched=false` 的结果。
- `services`：可选；服务名由 core 解析（非法值会返回错误）。
- `timeoutMs`：可选；最小 `1`（ms）。

result：`LyricsSearchResult[]`（按 `quality` 排序，best-effort）

示例元素（字段形状）：

```json
{
  "service": "qq",
  "serviceToken": "003rJQ7o3S0YdK",
  "title": "Hello",
  "artist": "Adele",
  "album": "Hello",
  "durationMs": 296000,
  "matchPercentage": 100,
  "quality": 1.23,
  "matched": true,
  "hasTranslation": true,
  "hasInlineTimetags": false,
  "lyricsOriginal": "[00:01.00] ...",
  "lyricsTranslation": "[00:01.00] ...",
  "debug": null
}
```

## 音乐下载（歌曲搜索 / 登录 / 下载会话）

说明：
- 字段形状与 `chaos-proto` 的 music DTO 对齐（`camelCase`）。
- `service` 取值：`qq | kugou | netease | kuwo`。
- 客户端负责保存登录态（Cookie/Token）；daemon 不落盘，只接收/返回。
- 合规边界：仅下载“接口返回的可直接下载 URL”，不包含任何 DRM 解密/绕过逻辑；若无权限/登录失效/接口不返回 URL，会返回明确错误。

### `music.config.set`（注入 ProviderConfig）

params：`MusicProviderConfig`

```json
{
  "kugouBaseUrl": "http://127.0.0.1:3000",
  "neteaseBaseUrls": ["http://127.0.0.1:3001"],
  "neteaseAnonymousCookieUrl": "/register/anonimous"
}
```

result：`OkReply`

```json
{ "ok": true }
```

说明：
- 酷狗：需要配置 `kugouBaseUrl` 才能启用相关能力（留空则不可用）。
- 网易云：若 `neteaseBaseUrls` 为空，daemon 会使用内置的一组可用 API 列表；也可通过 `music.config.set` 覆盖。

### `music.searchTracks` / `music.searchAlbums` / `music.searchArtists`

params：`MusicSearchParams`

```json
{ "service": "qq", "keyword": "周杰伦", "page": 1, "pageSize": 20 }
```

result：
- `music.searchTracks` -> `MusicTrack[]`
- `music.searchAlbums` -> `MusicAlbum[]`
- `music.searchArtists` -> `MusicArtist[]`

### `music.albumTracks` / `music.artistAlbums`

params：

```json
{ "service": "qq", "albumId": "123" }
```

```json
{ "service": "qq", "artistId": "456" }
```

result：
- `music.albumTracks` -> `MusicTrack[]`
- `music.artistAlbums` -> `MusicAlbum[]`

### `music.trackPlayUrl`

用于获取“可直接播放/试听”的 URL（best-effort）。常见用途：
- UI 点播放预览
- 下载前做一次“能否拿到 URL / 是否需要登录”的探测

params：`MusicTrackPlayUrlParams`

```json
{
  "service": "qq",
  "trackId": "songmid",
  "qualityId": "flac",
  "auth": { "qq": null, "kugou": null, "neteaseCookie": null }
}
```

result：`MusicTrackPlayUrlResult`

```json
{ "url": "https://...", "ext": "flac" }
```

### `music.qq.loginQrCreate` / `music.qq.loginQrPoll`

创建二维码：

params：`MusicLoginQrCreateParams`

```json
{ "loginType": "qq" }
```

result：`MusicLoginQr`（`base64` 为二维码图片）

轮询：

params：`MusicLoginQrPollParams`

```json
{ "sessionId": "<sessionId>" }
```

result：`MusicLoginQrPollResult`

说明：
- `state`：`scan | confirm | done | timeout | refuse | other`
- 成功时 `cookie` 非空（`QqMusicCookie`），客户端应保存并在下载时传回。

### `music.qq.refreshCookie`

params：

```json
{ "cookie": { "strMusicid": "...", "musickey": "...", "refreshKey": "...", "loginType": 1 } }
```

result：`QqMusicCookie`

### `music.kugou.loginQrCreate` / `music.kugou.loginQrPoll`

说明：依赖 `kugouBaseUrl` 配置；成功时 `kugouUser`（token/userid）非空。

### `music.download.start` / `music.download.status` / `music.download.cancel`

start params：`MusicDownloadStartParams`（包含 `config` + `auth` + `target` + `options`）

示例（单曲）：

```json
{
  "config": { "kugouBaseUrl": null, "neteaseBaseUrls": [], "neteaseAnonymousCookieUrl": "/register/anonimous" },
  "auth": { "qq": { "strMusicid": "...", "musickey": "...", "refreshKey": "...", "loginType": 1 }, "kugou": null, "neteaseCookie": null },
  "target": { "type": "track", "track": { "service": "qq", "id": "songmid", "title": "Song", "artists": ["Artist"], "artistIds": [], "album": "Album", "albumId": "1", "qualities": [] } },
  "options": { "qualityId": "flac", "outDir": "D:/Music", "overwrite": false, "concurrency": 3, "retries": 2 }
}
```

start result：`MusicDownloadStartResult`

```json
{ "sessionId": "<downloadSessionId>" }
```

status params：

```json
{ "sessionId": "<downloadSessionId>" }
```

status result：`MusicDownloadStatus`（含 totals + jobs 状态）

cancel params：

```json
{ "sessionId": "<downloadSessionId>" }
```

cancel result：`OkReply`

### `livestream.decodeManifest`（直播源解析 / 清晰度列表）

params：

```json
{ "input": "https://live.bilibili.com/1" }
```

result：`LivestreamDecodeManifestResult`

```json
{
  "site": "bili_live",
  "roomId": "1",
  "rawInput": "https://live.bilibili.com/1",
  "info": {
    "title": "...",
    "name": "...",
    "avatar": "...",
    "cover": "...",
    "isLiving": true
  },
  "playback": { "referer": "https://live.bilibili.com/", "userAgent": null },
  "variants": [
    {
      "id": "bili_live:2000:原画",
      "label": "原画",
      "quality": 2000,
      "rate": null,
      "url": "https://...optional...",
      "backupUrls": []
    }
  ]
}
```

说明：
- `variants[i].url` 可能为 `null`（平台需要二段解析时由 `live.open` 内部补齐，或由客户端自行走 FFI/其它链路补齐）。
- `playback.referer/userAgent` 建议作为播放器请求头配置。

### `live.open`（打开直播：选清晰度 + 获取可播 URL + 自动连弹幕）

params：

```json
{
  "input": "https://live.bilibili.com/1",
  "preferredQuality": "highest",
  "variantId": "bili_live:2000:原画"
}
```

字段说明：
- `input`：必填；支持完整 URL 或平台前缀（复用 core 的解析规则）。
- `preferredQuality`：可选；`"highest"`（默认）或 `"lowest"`。仅当未指定 `variantId` 时生效。
- `variantId`：可选；指定后会强制选择该清晰度（找不到会报错）。

result：`LiveOpenResult`

```json
{
  "sessionId": "uuid...",
  "site": "bili_live",
  "roomId": "1",
  "title": "直播标题",
  "variantId": "bili_live:2000:原画",
  "variantLabel": "原画",
  "url": "https://...m3u8/flv...",
  "backupUrls": ["https://..."],
  "referer": "https://live.bilibili.com/",
  "userAgent": null
}
```

说明：
- 成功后 daemon 会开始推送 `danmaku.message` 通知（`params.sessionId` = `result.sessionId`）。
- 如果该连接上已有 live session，会先隐式关闭旧 session（见“会话模型”）。

### `live.close`（关闭直播/弹幕会话）

params：

```json
{ "sessionId": "uuid..." }
```

result：`OkReply`

```json
{ "ok": true }
```

说明：
- `sessionId` 不存在会返回 `InternalError`（例如 `SessionNotFound`）。

### `danmaku.connect`（只连接弹幕，不解析直播 URL）

params：

```json
{ "input": "https://live.bilibili.com/1" }
```

result：`DanmakuConnectResult`

```json
{ "sessionId": "uuid...", "site": "bili_live", "roomId": "1" }
```

说明：
- 成功后 daemon 会开始推送 `danmaku.message` 通知（`params.sessionId` = `result.sessionId`）。
- 如果该连接上已有 danmaku session，会先隐式断开旧 session（见“会话模型”）。

### `danmaku.disconnect`

params：

```json
{ "sessionId": "uuid..." }
```

result：`OkReply`

```json
{ "ok": true }
```

### `danmaku.fetchImage`（拉取弹幕图片并 base64 返回）

用于把弹幕消息里的 `imageUrl` 拉取并编码为 base64，方便 WinUI3 直接显示。

params：

```json
{ "sessionId": "uuid...", "url": "https://.../img.png" }
```

result：`DanmakuFetchImageResult`

```json
{ "mime": "image/png", "base64": "...", "width": 128 }
```

安全/限制（会导致 `InternalError`）：
- 只允许 `http/https`。
- 会阻止访问本机/内网/私有地址（避免 SSRF）。
- 响应过大时会拒绝（上限由 daemon 配置控制）。

### `liveDir.categories`

params：

```json
{ "site": "bili_live" }
```

result：`LiveDirCategory[]`

```json
[
  {
    "id": "1",
    "name": "网游",
    "children": [
      { "id": "11", "parentId": "1", "name": "英雄联盟", "pic": "https://...@100w.png" }
    ]
  }
]
```

### `liveDir.recommendRooms`

params：

```json
{ "site": "bili_live", "page": 1 }
```

result：`LiveDirRoomListResult`

```json
{
  "hasMore": true,
  "items": [
    {
      "site": "bili_live",
      "roomId": "100",
      "input": "bilibili:100",
      "title": "标题",
      "cover": "https://...@400w.jpg",
      "userName": "主播",
      "online": 12345
    }
  ]
}
```

### `liveDir.categoryRooms`

params：

```json
{
  "site": "bili_live",
  "parentId": "1",
  "categoryId": "11",
  "page": 1
}
```

说明：
- B 站需要 `parentId + categoryId`。
- 虎牙/斗鱼通常只需要 `categoryId`；`parentId` 可为 `null`。

result：`LiveDirRoomListResult`（同上）。

说明（BiliLive 风控）：
- 若遇到 `error.code = -32603` 且 `error.message` 中包含 B 站 `code = -352/-412`，通常表示请求被拦截/设备校验。
- core 会做 best-effort 的自动重试/回退；如果仍失败，请稍后重试或更换网络环境。

### `liveDir.searchRooms`

params：

```json
{ "site": "bili_live", "keyword": "lol", "page": 1 }
```

result：`LiveDirRoomListResult`（同上）。

## 通知（server -> client）

### `danmaku.message`

payload（notification，无 `id`，`params` = `DanmakuMessage`）：

```json
{
  "sessionId": "uuid...",
  "receivedAtMs": 1700000000000,
  "user": "username",
  "text": "hello",
  "imageUrl": "https://...optional...",
  "imageWidth": 128
}
```

说明：
- `sessionId` 用于把消息路由到对应 UI 会话。
- 图片弹幕：`imageUrl`/`imageWidth` 可能存在；可用 `danmaku.fetchImage` 拉取 base64。
