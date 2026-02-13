# chaos-daemon API（JSON-RPC / 中文说明）

`chaos-daemon` 是 Windows 下的后端进程：负责把 `chaos-core` / `chaos-app` 的能力通过 **NamedPipe + JSON-RPC 2.0** 暴露给 WinUI3（或任何 .NET 客户端）。

传输层使用 **LSP Content-Length framing**（也就是每条 JSON-RPC 消息外包一层 `Content-Length: ...\r\n\r\n<json>`）。

## 连接与鉴权

daemon 启动参数：

```txt
chaos-daemon.exe --pipe-name <PIPE_NAME> --auth-token <TOKEN>
```

连接成功后，客户端必须先调用一次 `daemon.ping` 完成鉴权：

- 在 daemon **未鉴权**前，除 `daemon.ping` 外的所有方法会返回 `Unauthorized`。
- `authToken` 不匹配时，daemon 会返回 `Unauthorized` 并断开（客户端应重启/重连）。

## 通用约定

- 所有方法均为 JSON-RPC 2.0 request/response。
- 参数使用 `params: { ... }`（对象参数）。
- 字段命名为 `camelCase`。
- `site` 约定值：`bili_live` / `huya` / `douyu`（与 WinUI3、FFI 端一致）。

## 方法一览（含新增 Live Directory）

### `daemon.ping`

params:

```json
{ "authToken": "<TOKEN>" }
```

result:

```json
{ "version": "0.3.0" }
```

### `liveDir.categories`（新增）

params:

```json
{ "site": "bili_live" }
```

result: `LiveDirCategory[]`

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

### `liveDir.recommendRooms`（新增）

params:

```json
{ "site": "bili_live", "page": 1 }
```

result: `LiveDirRoomListResult`

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

### `liveDir.categoryRooms`（新增）

params:

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

result: `LiveDirRoomListResult`（同上）。

### `liveDir.searchRooms`（新增）

params:

```json
{ "site": "bili_live", "keyword": "lol", "page": 1 }
```

result: `LiveDirRoomListResult`（同上）。

## Rust 侧（服务方法签名）

daemon 内部通过 `ChaosService` trait 暴露能力（省略其它方法）：

```rust
async fn live_dir_categories(params: LiveDirCategoriesParams) -> Result<Vec<LiveDirCategory>, String>;
async fn live_dir_recommend_rooms(params: LiveDirRecommendRoomsParams) -> Result<LiveDirRoomListResult, String>;
async fn live_dir_category_rooms(params: LiveDirCategoryRoomsParams) -> Result<LiveDirRoomListResult, String>;
async fn live_dir_search_rooms(params: LiveDirSearchRoomsParams) -> Result<LiveDirRoomListResult, String>;
```

## 备注

daemon 的其它方法（直播解析/弹幕/歌词/NowPlaying 等）以 `chaos-proto` 中的方法名与 DTO 为准：

- `livestream.decodeManifest`
- `live.open` / `live.close`
- `danmaku.connect` / `danmaku.disconnect` / `danmaku.fetchImage` + 通知 `danmaku.message`
- `lyrics.search`
- `nowPlaying.snapshot`
