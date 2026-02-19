using System.Text.Json.Serialization;
using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Models.Bili;

public sealed class BiliAuthState
{
    [JsonProperty("cookie")]
    [JsonPropertyName("cookie")]
    public string? Cookie { get; set; }

    [JsonProperty("refreshToken")]
    [JsonPropertyName("refreshToken")]
    public string? RefreshToken { get; set; }
}

public sealed class BiliWebAuth
{
    [JsonProperty("cookie")]
    [JsonPropertyName("cookie")]
    public string Cookie { get; set; } = "";

    [JsonProperty("refreshToken")]
    [JsonPropertyName("refreshToken")]
    public string? RefreshToken { get; set; }
}

public sealed class BiliTvAuth
{
    [JsonProperty("accessToken")]
    [JsonPropertyName("accessToken")]
    public string AccessToken { get; set; } = "";
}

public sealed class BiliAuthBundle
{
    [JsonProperty("web")]
    [JsonPropertyName("web")]
    public BiliWebAuth? Web { get; set; }

    [JsonProperty("tv")]
    [JsonPropertyName("tv")]
    public BiliTvAuth? Tv { get; set; }
}

public sealed class BiliLoginQrCreateV2Params
{
    [JsonProperty("loginType")]
    [JsonPropertyName("loginType")]
    public string LoginType { get; set; } = "web"; // "web" | "tv"
}

public sealed class BiliLoginQrPollResultV2
{
    [JsonProperty("sessionId")]
    [JsonPropertyName("sessionId")]
    public string SessionId { get; set; } = "";

    [JsonProperty("state")]
    [JsonPropertyName("state")]
    public string State { get; set; } = "";

    [JsonProperty("message")]
    [JsonPropertyName("message")]
    public string? Message { get; set; }

    [JsonProperty("auth")]
    [JsonPropertyName("auth")]
    public BiliAuthBundle? Auth { get; set; }
}

public sealed class BiliCheckLoginParams
{
    [JsonProperty("auth")]
    [JsonPropertyName("auth")]
    public BiliAuthBundle Auth { get; set; } = new();
}

public sealed class BiliCheckLoginResult
{
    [JsonProperty("isLogin")]
    [JsonPropertyName("isLogin")]
    public bool IsLogin { get; set; }

    [JsonProperty("reason")]
    [JsonPropertyName("reason")]
    public string? Reason { get; set; }

    [JsonProperty("missingFields")]
    [JsonPropertyName("missingFields")]
    public string[] MissingFields { get; set; } = Array.Empty<string>();
}

public sealed class BiliLoginQr
{
    [JsonProperty("sessionId")]
    [JsonPropertyName("sessionId")]
    public string SessionId { get; set; } = "";

    [JsonProperty("mime")]
    [JsonPropertyName("mime")]
    public string Mime { get; set; } = "";

    [JsonProperty("base64")]
    [JsonPropertyName("base64")]
    public string Base64 { get; set; } = "";

    [JsonProperty("url")]
    [JsonPropertyName("url")]
    public string Url { get; set; } = "";

    [JsonProperty("qrcodeKey")]
    [JsonPropertyName("qrcodeKey")]
    public string QrcodeKey { get; set; } = "";

    [JsonProperty("createdAtUnixMs")]
    [JsonPropertyName("createdAtUnixMs")]
    public long CreatedAtUnixMs { get; set; }
}

public sealed class BiliLoginQrPollResult
{
    [JsonProperty("sessionId")]
    [JsonPropertyName("sessionId")]
    public string SessionId { get; set; } = "";

    [JsonProperty("state")]
    [JsonPropertyName("state")]
    public string State { get; set; } = "";

    [JsonProperty("message")]
    [JsonPropertyName("message")]
    public string? Message { get; set; }

    [JsonProperty("auth")]
    [JsonPropertyName("auth")]
    public BiliAuthState? Auth { get; set; }
}

public sealed class BiliRefreshCookieParams
{
    [JsonProperty("auth")]
    [JsonPropertyName("auth")]
    public BiliAuthState Auth { get; set; } = new();
}

public sealed class BiliRefreshCookieResult
{
    [JsonProperty("auth")]
    [JsonPropertyName("auth")]
    public BiliAuthState Auth { get; set; } = new();
}

public sealed class BiliParseParams
{
    [JsonProperty("input")]
    [JsonPropertyName("input")]
    public string Input { get; set; } = "";

    [JsonProperty("auth")]
    [JsonPropertyName("auth")]
    public BiliAuthState? Auth { get; set; }
}

public sealed class BiliPage
{
    [JsonProperty("pageNumber")]
    [JsonPropertyName("pageNumber")]
    public uint PageNumber { get; set; }

    [JsonProperty("cid")]
    [JsonPropertyName("cid")]
    public string Cid { get; set; } = "";

    [JsonProperty("pageTitle")]
    [JsonPropertyName("pageTitle")]
    public string PageTitle { get; set; } = "";

    [JsonProperty("durationS")]
    [JsonPropertyName("durationS")]
    public uint? DurationS { get; set; }

    [JsonProperty("dimension")]
    [JsonPropertyName("dimension")]
    public string? Dimension { get; set; }
}

public sealed class BiliParsedVideo
{
    [JsonProperty("aid")]
    [JsonPropertyName("aid")]
    public string Aid { get; set; } = "";

    [JsonProperty("bvid")]
    [JsonPropertyName("bvid")]
    public string Bvid { get; set; } = "";

    [JsonProperty("title")]
    [JsonPropertyName("title")]
    public string Title { get; set; } = "";

    [JsonProperty("desc")]
    [JsonPropertyName("desc")]
    public string? Desc { get; set; }

    [JsonProperty("pic")]
    [JsonPropertyName("pic")]
    public string? Pic { get; set; }

    [JsonProperty("ownerName")]
    [JsonPropertyName("ownerName")]
    public string? OwnerName { get; set; }

    [JsonProperty("ownerMid")]
    [JsonPropertyName("ownerMid")]
    public string? OwnerMid { get; set; }

    [JsonProperty("pubTimeUnixS")]
    [JsonPropertyName("pubTimeUnixS")]
    public long? PubTimeUnixS { get; set; }

    [JsonProperty("pages")]
    [JsonPropertyName("pages")]
    public BiliPage[] Pages { get; set; } = Array.Empty<BiliPage>();
}

public sealed class BiliParseResult
{
    [JsonProperty("videos")]
    [JsonPropertyName("videos")]
    public BiliParsedVideo[] Videos { get; set; } = Array.Empty<BiliParsedVideo>();
}

public sealed class BiliDownloadOptions
{
    [JsonProperty("outDir")]
    [JsonPropertyName("outDir")]
    public string OutDir { get; set; } = "";

    [JsonProperty("selectPage")]
    [JsonPropertyName("selectPage")]
    public string SelectPage { get; set; } = "ALL";

    [JsonProperty("dfnPriority")]
    [JsonPropertyName("dfnPriority")]
    public string DfnPriority { get; set; } = "";

    [JsonProperty("encodingPriority")]
    [JsonPropertyName("encodingPriority")]
    public string EncodingPriority { get; set; } = "hevc,av1,avc";

    [JsonProperty("filePattern")]
    [JsonPropertyName("filePattern")]
    public string FilePattern { get; set; } = "<videoTitle>";

    [JsonProperty("multiFilePattern")]
    [JsonPropertyName("multiFilePattern")]
    public string MultiFilePattern { get; set; } = "<videoTitle>/[P<pageNumberWithZero>]<pageTitle>";

    [JsonProperty("downloadSubtitle")]
    [JsonPropertyName("downloadSubtitle")]
    public bool DownloadSubtitle { get; set; } = true;

    [JsonProperty("skipMux")]
    [JsonPropertyName("skipMux")]
    public bool SkipMux { get; set; } = false;

    [JsonProperty("concurrency")]
    [JsonPropertyName("concurrency")]
    public uint Concurrency { get; set; } = 4;

    [JsonProperty("retries")]
    [JsonPropertyName("retries")]
    public uint Retries { get; set; } = 2;

    [JsonProperty("ffmpegPath")]
    [JsonPropertyName("ffmpegPath")]
    public string FfmpegPath { get; set; } = "";
}

public sealed class BiliDownloadStartParams
{
    [JsonProperty("api")]
    [JsonPropertyName("api")]
    public string Api { get; set; } = "web";

    [JsonProperty("input")]
    [JsonPropertyName("input")]
    public string Input { get; set; } = "";

    [JsonProperty("auth")]
    [JsonPropertyName("auth")]
    public BiliAuthState? Auth { get; set; }

    [JsonProperty("options")]
    [JsonPropertyName("options")]
    public BiliDownloadOptions Options { get; set; } = new();
}

public sealed class BiliDownloadStartResult
{
    [JsonProperty("sessionId")]
    [JsonPropertyName("sessionId")]
    public string SessionId { get; set; } = "";
}

public sealed class BiliDownloadTotals
{
    [JsonProperty("total")]
    [JsonPropertyName("total")]
    public uint Total { get; set; }

    [JsonProperty("done")]
    [JsonPropertyName("done")]
    public uint Done { get; set; }

    [JsonProperty("failed")]
    [JsonPropertyName("failed")]
    public uint Failed { get; set; }

    [JsonProperty("skipped")]
    [JsonPropertyName("skipped")]
    public uint Skipped { get; set; }

    [JsonProperty("canceled")]
    [JsonPropertyName("canceled")]
    public uint Canceled { get; set; }
}

public sealed class BiliDownloadJobStatus
{
    [JsonProperty("index")]
    [JsonPropertyName("index")]
    public uint Index { get; set; }

    [JsonProperty("pageNumber")]
    [JsonPropertyName("pageNumber")]
    public uint? PageNumber { get; set; }

    [JsonProperty("cid")]
    [JsonPropertyName("cid")]
    public string? Cid { get; set; }

    [JsonProperty("title")]
    [JsonPropertyName("title")]
    public string Title { get; set; } = "";

    [JsonProperty("state")]
    [JsonPropertyName("state")]
    public string State { get; set; } = "";

    [JsonProperty("phase")]
    [JsonPropertyName("phase")]
    public string Phase { get; set; } = "";

    [JsonProperty("bytesDownloaded")]
    [JsonPropertyName("bytesDownloaded")]
    public ulong BytesDownloaded { get; set; }

    [JsonProperty("bytesTotal")]
    [JsonPropertyName("bytesTotal")]
    public ulong? BytesTotal { get; set; }

    [JsonProperty("speedBps")]
    [JsonPropertyName("speedBps")]
    public ulong? SpeedBps { get; set; }

    [JsonProperty("path")]
    [JsonPropertyName("path")]
    public string? Path { get; set; }

    [JsonProperty("error")]
    [JsonPropertyName("error")]
    public string? Error { get; set; }
}

public sealed class BiliDownloadStatus
{
    [JsonProperty("done")]
    [JsonPropertyName("done")]
    public bool Done { get; set; }

    [JsonProperty("totals")]
    [JsonPropertyName("totals")]
    public BiliDownloadTotals Totals { get; set; } = new();

    [JsonProperty("jobs")]
    [JsonPropertyName("jobs")]
    public BiliDownloadJobStatus[] Jobs { get; set; } = Array.Empty<BiliDownloadJobStatus>();
}

public sealed class BiliDownloadStatusParams
{
    [JsonProperty("sessionId")]
    [JsonPropertyName("sessionId")]
    public string SessionId { get; set; } = "";
}

public sealed class BiliDownloadCancelParams
{
    [JsonProperty("sessionId")]
    [JsonPropertyName("sessionId")]
    public string SessionId { get; set; } = "";
}

public sealed class BiliTaskAddParams
{
    [JsonProperty("api")]
    [JsonPropertyName("api")]
    public string Api { get; set; } = "auto";

    [JsonProperty("input")]
    [JsonPropertyName("input")]
    public string Input { get; set; } = "";

    [JsonProperty("auth")]
    [JsonPropertyName("auth")]
    public BiliAuthBundle? Auth { get; set; }

    [JsonProperty("options")]
    [JsonPropertyName("options")]
    public BiliDownloadOptions Options { get; set; } = new();
}

public sealed class BiliTaskAddResult
{
    [JsonProperty("taskId")]
    [JsonPropertyName("taskId")]
    public string TaskId { get; set; } = "";
}

public sealed class BiliTask
{
    [JsonProperty("taskId")]
    [JsonPropertyName("taskId")]
    public string TaskId { get; set; } = "";

    [JsonProperty("input")]
    [JsonPropertyName("input")]
    public string Input { get; set; } = "";

    [JsonProperty("api")]
    [JsonPropertyName("api")]
    public string Api { get; set; } = "auto";

    [JsonProperty("createdAtUnixMs")]
    [JsonPropertyName("createdAtUnixMs")]
    public long CreatedAtUnixMs { get; set; }

    [JsonProperty("done")]
    [JsonPropertyName("done")]
    public bool Done { get; set; }

    [JsonProperty("totals")]
    [JsonPropertyName("totals")]
    public BiliDownloadTotals Totals { get; set; } = new();
}

public sealed class BiliTasksGetParams { }

public sealed class BiliTasksGetResult
{
    [JsonProperty("running")]
    [JsonPropertyName("running")]
    public BiliTask[] Running { get; set; } = Array.Empty<BiliTask>();

    [JsonProperty("finished")]
    [JsonPropertyName("finished")]
    public BiliTask[] Finished { get; set; } = Array.Empty<BiliTask>();
}

public sealed class BiliTaskGetParams
{
    [JsonProperty("taskId")]
    [JsonPropertyName("taskId")]
    public string TaskId { get; set; } = "";
}

public sealed class BiliTaskDetail
{
    [JsonProperty("task")]
    [JsonPropertyName("task")]
    public BiliTask Task { get; set; } = new();

    [JsonProperty("status")]
    [JsonPropertyName("status")]
    public BiliDownloadStatus Status { get; set; } = new();
}

public sealed class BiliTaskCancelParams
{
    [JsonProperty("taskId")]
    [JsonPropertyName("taskId")]
    public string TaskId { get; set; } = "";
}

public sealed class BiliTasksRemoveFinishedParams
{
    [JsonProperty("onlyFailed")]
    [JsonPropertyName("onlyFailed")]
    public bool? OnlyFailed { get; set; }

    [JsonProperty("taskId")]
    [JsonPropertyName("taskId")]
    public string? TaskId { get; set; }
}
