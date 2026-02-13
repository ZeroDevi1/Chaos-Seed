using Newtonsoft.Json;
using System.Text.Json.Serialization;

namespace ChaosSeed.WinUI3.Models.Music;

public sealed class MusicProviderConfig
{
    [JsonProperty("kugouBaseUrl")]
    public string? KugouBaseUrl { get; set; }

    [JsonProperty("neteaseBaseUrls")]
    public string[] NeteaseBaseUrls { get; set; } = Array.Empty<string>();

    [JsonProperty("neteaseAnonymousCookieUrl")]
    public string? NeteaseAnonymousCookieUrl { get; set; }
}

public sealed class MusicQuality
{
    [JsonProperty("id")]
    public string Id { get; set; } = "";

    [JsonProperty("label")]
    public string Label { get; set; } = "";

    [JsonProperty("format")]
    public string Format { get; set; } = "";

    [JsonProperty("bitrateKbps")]
    public uint? BitrateKbps { get; set; }

    [JsonProperty("lossless")]
    public bool Lossless { get; set; }
}

public sealed class MusicTrack
{
    [JsonProperty("service")]
    public string Service { get; set; } = "";

    [JsonProperty("id")]
    public string Id { get; set; } = "";

    [JsonProperty("title")]
    public string Title { get; set; } = "";

    [JsonProperty("artists")]
    public string[] Artists { get; set; } = Array.Empty<string>();

    [JsonProperty("artistIds")]
    public string[] ArtistIds { get; set; } = Array.Empty<string>();

    [JsonProperty("album")]
    public string? Album { get; set; }

    [JsonProperty("albumId")]
    public string? AlbumId { get; set; }

    [JsonProperty("durationMs")]
    public ulong? DurationMs { get; set; }

    [JsonProperty("coverUrl")]
    public string? CoverUrl { get; set; }

    [JsonProperty("qualities")]
    public MusicQuality[] Qualities { get; set; } = Array.Empty<MusicQuality>();
}

public sealed class MusicAlbum
{
    [JsonProperty("service")]
    public string Service { get; set; } = "";

    [JsonProperty("id")]
    public string Id { get; set; } = "";

    [JsonProperty("title")]
    public string Title { get; set; } = "";

    [JsonProperty("artist")]
    public string? Artist { get; set; }

    [JsonProperty("artistId")]
    public string? ArtistId { get; set; }

    [JsonProperty("coverUrl")]
    public string? CoverUrl { get; set; }

    [JsonProperty("publishTime")]
    public string? PublishTime { get; set; }

    [JsonProperty("trackCount")]
    public uint? TrackCount { get; set; }
}

public sealed class MusicArtist
{
    [JsonProperty("service")]
    public string Service { get; set; } = "";

    [JsonProperty("id")]
    public string Id { get; set; } = "";

    [JsonProperty("name")]
    public string Name { get; set; } = "";

    [JsonProperty("coverUrl")]
    public string? CoverUrl { get; set; }

    [JsonProperty("albumCount")]
    public uint? AlbumCount { get; set; }
}

public sealed class MusicSearchParams
{
    [JsonProperty("service")]
    public string Service { get; set; } = "";

    [JsonProperty("keyword")]
    public string Keyword { get; set; } = "";

    [JsonProperty("page")]
    public uint Page { get; set; } = 1;

    [JsonProperty("pageSize")]
    public uint PageSize { get; set; } = 20;
}

public sealed class MusicAlbumTracksParams
{
    [JsonProperty("service")]
    public string Service { get; set; } = "";

    [JsonProperty("albumId")]
    public string AlbumId { get; set; } = "";
}

public sealed class MusicArtistAlbumsParams
{
    [JsonProperty("service")]
    public string Service { get; set; } = "";

    [JsonProperty("artistId")]
    public string ArtistId { get; set; } = "";
}

public sealed class QqMusicCookie
{
    [JsonProperty("openid")]
    public string? Openid { get; set; }

    [JsonProperty("refreshToken")]
    public string? RefreshToken { get; set; }

    [JsonProperty("accessToken")]
    public string? AccessToken { get; set; }

    [JsonProperty("expiredAt")]
    public long? ExpiredAt { get; set; }

    [JsonProperty("musicid")]
    public string? Musicid { get; set; }

    [JsonProperty("musickey")]
    public string? Musickey { get; set; }

    [JsonProperty("musickeyCreateTime")]
    public long? MusickeyCreateTime { get; set; }

    [JsonProperty("firstLogin")]
    public long? FirstLogin { get; set; }

    [JsonProperty("refreshKey")]
    public string? RefreshKey { get; set; }

    [JsonProperty("loginType")]
    public long? LoginType { get; set; }

    [JsonProperty("strMusicid")]
    public string? StrMusicid { get; set; }

    [JsonProperty("nick")]
    public string? Nick { get; set; }

    [JsonProperty("logo")]
    public string? Logo { get; set; }

    [JsonProperty("encryptUin")]
    public string? EncryptUin { get; set; }
}

public sealed class KugouUserInfo
{
    [JsonProperty("token")]
    public string Token { get; set; } = "";

    [JsonProperty("userid")]
    public string Userid { get; set; } = "";
}

public sealed class MusicAuthState
{
    [JsonProperty("qq")]
    public QqMusicCookie? Qq { get; set; }

    [JsonProperty("kugou")]
    public KugouUserInfo? Kugou { get; set; }

    [JsonProperty("neteaseCookie")]
    public string? NeteaseCookie { get; set; }
}

public sealed class MusicLoginQr
{
    [JsonProperty("sessionId")]
    public string SessionId { get; set; } = "";

    [JsonProperty("loginType")]
    public string LoginType { get; set; } = "";

    [JsonProperty("mime")]
    public string Mime { get; set; } = "";

    [JsonProperty("base64")]
    public string Base64 { get; set; } = "";

    // identifier + createdAtUnixMs are present for debugging; UI can ignore.
    [JsonProperty("identifier")]
    public string? Identifier { get; set; }

    [JsonProperty("createdAtUnixMs")]
    public long? CreatedAtUnixMs { get; set; }
}

public sealed class MusicLoginQrPollResult
{
    [JsonProperty("sessionId")]
    public string SessionId { get; set; } = "";

    [JsonProperty("state")]
    public string State { get; set; } = "";

    [JsonProperty("message")]
    public string? Message { get; set; }

    [JsonProperty("cookie")]
    public QqMusicCookie? Cookie { get; set; }

    [JsonProperty("kugouUser")]
    public KugouUserInfo? KugouUser { get; set; }
}

public sealed class MusicDownloadOptions
{
    [JsonProperty("qualityId")]
    public string QualityId { get; set; } = "mp3_320";

    [JsonProperty("outDir")]
    public string OutDir { get; set; } = "";

    [JsonProperty("pathTemplate")]
    [JsonPropertyName("pathTemplate")]
    public string? PathTemplate { get; set; }

    [JsonProperty("overwrite")]
    public bool Overwrite { get; set; }

    [JsonProperty("concurrency")]
    public uint Concurrency { get; set; } = 3;

    [JsonProperty("retries")]
    public uint Retries { get; set; } = 2;
}

public sealed class MusicTrackPlayUrlParams
{
    [JsonProperty("service")]
    [JsonPropertyName("service")]
    public string Service { get; set; } = "";

    [JsonProperty("trackId")]
    [JsonPropertyName("trackId")]
    public string TrackId { get; set; } = "";

    [JsonProperty("qualityId")]
    [JsonPropertyName("qualityId")]
    public string? QualityId { get; set; }

    [JsonProperty("auth")]
    [JsonPropertyName("auth")]
    public MusicAuthState Auth { get; set; } = new();
}

public sealed class MusicTrackPlayUrlResult
{
    [JsonProperty("url")]
    [JsonPropertyName("url")]
    public string Url { get; set; } = "";

    [JsonProperty("ext")]
    [JsonPropertyName("ext")]
    public string Ext { get; set; } = "";
}

public sealed class MusicDownloadStartParams
{
    [JsonProperty("config")]
    public MusicProviderConfig Config { get; set; } = new();

    [JsonProperty("auth")]
    public MusicAuthState Auth { get; set; } = new();

    // Tagged union: { type: "track", track: {...} } / { type: "album", service, albumId } / { type: "artist_all", service, artistId }
    [JsonProperty("target")]
    public object Target { get; set; } = new();

    [JsonProperty("options")]
    public MusicDownloadOptions Options { get; set; } = new();
}

public sealed class MusicDownloadStartResult
{
    [JsonProperty("sessionId")]
    public string SessionId { get; set; } = "";
}

public sealed class MusicDownloadTotals
{
    [JsonProperty("total")]
    public uint Total { get; set; }

    [JsonProperty("done")]
    public uint Done { get; set; }

    [JsonProperty("failed")]
    public uint Failed { get; set; }

    [JsonProperty("skipped")]
    public uint Skipped { get; set; }

    [JsonProperty("canceled")]
    public uint Canceled { get; set; }
}

public sealed class MusicDownloadJobResult
{
    [JsonProperty("index")]
    public uint Index { get; set; }

    [JsonProperty("trackId")]
    public string? TrackId { get; set; }

    [JsonProperty("state")]
    public string State { get; set; } = "";

    [JsonProperty("path")]
    public string? Path { get; set; }

    [JsonProperty("bytes")]
    public ulong? Bytes { get; set; }

    [JsonProperty("error")]
    public string? Error { get; set; }
}

public sealed class MusicDownloadStatus
{
    [JsonProperty("done")]
    public bool Done { get; set; }

    [JsonProperty("totals")]
    public MusicDownloadTotals Totals { get; set; } = new();

    [JsonProperty("jobs")]
    public MusicDownloadJobResult[] Jobs { get; set; } = Array.Empty<MusicDownloadJobResult>();
}

public sealed class OkReply
{
    [JsonProperty("ok")]
    public bool Ok { get; set; }
}
