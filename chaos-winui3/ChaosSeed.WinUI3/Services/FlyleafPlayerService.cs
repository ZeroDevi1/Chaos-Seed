using FlyleafLib;
using FlyleafLib.MediaPlayer;
using Microsoft.UI.Dispatching;

namespace ChaosSeed.WinUI3.Services;

public sealed class FlyleafPlayerService : IDisposable
{
    private readonly DispatcherQueue _dq;
    private readonly Config _config;
    private readonly Player _player;

    public FlyleafPlayerService(DispatcherQueue dispatcherQueue)
    {
        _dq = dispatcherQueue;
        if (!Engine.IsLoaded)
        {
            throw new InvalidOperationException(
                "Flyleaf Engine 未初始化。请确认 FFmpeg DLL 已放置在可执行文件同目录的 FFmpeg/ 下。"
            );
        }

        try
        {
            _config = new Config();
        }
        catch (Exception ex)
        {
            throw new InvalidOperationException(
                $"创建 Flyleaf Config 失败：{ex.Message}（通常是 FFmpeg DLL 缺失/不匹配）",
                ex
            );
        }

        try
        {
            if (_config.Player is not null)
            {
                _config.Player.AutoPlay = false;
            }
        }
        catch
        {
            // ignore - config surface may vary by Flyleaf version
        }

        try
        {
            _player = new Player(_config);
        }
        catch (Exception ex)
        {
            throw new InvalidOperationException(
                $"创建 Flyleaf Player 失败：{ex.Message}（通常是 FFmpeg DLL 缺失/不匹配）",
                ex
            );
        }
    }

    public Player Player => _player;

    public event EventHandler<string>? Error;
    public event EventHandler<string>? Info;

    public async Task PlayAsync(
        string site,
        string url,
        IReadOnlyList<string> backupUrls,
        string? referer,
        string? userAgent,
        CancellationToken ct = default
    )
    {
        Stop();

        ApplyHttpHints(referer, userAgent);

        var primary = (url ?? "").Trim();
        var backups = backupUrls
            .Select(u => (u ?? "").Trim())
            .Where(u => !string.IsNullOrWhiteSpace(u))
            .ToList();

        if (string.IsNullOrWhiteSpace(primary) && backups.Count == 0)
        {
            throw new InvalidOperationException("empty url");
        }

        var candidates = new List<string>();
        if (string.Equals(site?.Trim(), "bili_live", StringComparison.OrdinalIgnoreCase))
        {
            candidates.AddRange(backups);
            if (!string.IsNullOrWhiteSpace(primary))
            {
                candidates.Add(primary);
            }
        }
        else
        {
            if (!string.IsNullOrWhiteSpace(primary))
            {
                candidates.Add(primary);
            }
            candidates.AddRange(backups);
        }

        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        candidates = candidates.Where(u => seen.Add(u)).ToList();

        Exception? last = null;
        foreach (var u in candidates)
        {
            try
            {
                ct.ThrowIfCancellationRequested();
                Info?.Invoke(this, $"加载：{u}");
                await OpenOnceAsync(u, ct);
                _player.Play();
                return;
            }
            catch (Exception ex)
            {
                last = ex;
                Error?.Invoke(this, $"尝试播放失败：{ex.Message}");
            }
        }

        throw last ?? new Exception("play failed");
    }

    public void Stop()
    {
        try
        {
            _player.Stop();
        }
        catch
        {
            // ignore
        }
    }

    private void ApplyHttpHints(string? referer, string? userAgent)
    {
        try
        {
            var fmt = _config.Demuxer.FormatOpt;

            var ua = string.IsNullOrWhiteSpace(userAgent) ? null : userAgent.Trim();
            var rf = string.IsNullOrWhiteSpace(referer) ? null : referer.Trim();

            if (ua is not null)
            {
                fmt["user_agent"] = ua;
            }
            else
            {
                fmt.Remove("user_agent");
            }

            if (rf is not null)
            {
                fmt["referer"] = rf;
            }
            else
            {
                fmt.Remove("referer");
            }

            var headers = new List<string>();
            if (rf is not null)
            {
                headers.Add($"Referer: {rf}");
            }
            if (ua is not null)
            {
                headers.Add($"User-Agent: {ua}");
            }

            if (headers.Count > 0)
            {
                fmt["headers"] = string.Join("\r\n", headers) + "\r\n";
            }
            else
            {
                fmt.Remove("headers");
            }
        }
        catch
        {
            // ignore - format opt support may vary by Flyleaf version
        }
    }

    private Task OpenOnceAsync(string url, CancellationToken ct)
    {
        var tcs = new TaskCompletionSource<OpenCompletedArgs>(TaskCreationOptions.RunContinuationsAsynchronously);

        EventHandler<OpenCompletedArgs>? handler = null;
        handler = (_, args) =>
        {
            if (args is null || args.IsSubtitles)
            {
                return;
            }

            _player.OpenCompleted -= handler;

            if (args.Success)
            {
                tcs.TrySetResult(args);
            }
            else
            {
                tcs.TrySetException(new Exception(args.Error ?? "open failed"));
            }
        };

        _player.OpenCompleted += handler;

        try
        {
            _player.OpenAsync(url);
        }
        catch (Exception ex)
        {
            _player.OpenCompleted -= handler;
            tcs.TrySetException(ex);
        }

        if (ct.CanBeCanceled)
        {
            ct.Register(() =>
            {
                _player.OpenCompleted -= handler;
                tcs.TrySetCanceled(ct);
            });
        }

        return tcs.Task;
    }

    public void Dispose()
    {
        Stop();
        try
        {
            _player.Dispose();
        }
        catch
        {
            // ignore
        }
    }
}
