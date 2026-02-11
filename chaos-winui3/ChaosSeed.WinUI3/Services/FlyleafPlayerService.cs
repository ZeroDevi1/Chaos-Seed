using System.Threading;
using FlyleafLib;
using FlyleafLib.MediaPlayer;
using Microsoft.UI.Dispatching;

namespace ChaosSeed.WinUI3.Services;

public sealed class PlayOpenOptions
{
    public TimeSpan OpenTimeout { get; init; } = TimeSpan.FromSeconds(15);
    public int RetryPerUrl { get; init; } = 1;
    public TimeSpan RetryDelay { get; init; } = TimeSpan.FromMilliseconds(300);
}

public sealed class FlyleafPlayerService : IDisposable
{
    private readonly DispatcherQueue _dq;
    private readonly Config _config;
    private readonly Player _player;
    private readonly SemaphoreSlim _openGate = new(1, 1);

    public FlyleafPlayerService(DispatcherQueue dispatcherQueue)
    {
        _dq = dispatcherQueue ?? throw new ArgumentNullException(nameof(dispatcherQueue));

        if (!Engine.IsLoaded)
        {
            throw new InvalidOperationException(
                "Flyleaf Engine 未初始化。请确认 FFmpeg DLL 已放置在可执行文件同目录的 FFmpeg/ 下。"
            );
        }

        // Player / FlyleafHost 的渲染相关对象带有线程亲和性，强制要求在 UI 线程创建。
        // 如果这里不在 UI 线程创建，后续即使封送到 UI 调用也可能触发 0x8001010E。
        if (!_dq.HasThreadAccess)
        {
            throw new InvalidOperationException("FlyleafPlayerService 必须在 UI 线程创建（DispatcherQueue 线程）。");
        }

        try
        {
            _player = new Player();
            _config = _player.Config;
            _config.Player.AutoPlay = false;
        }
        catch (Exception ex)
        {
            throw new InvalidOperationException(
                $"创建 Flyleaf Player 失败：{ex.Message}（通常是 FFmpeg DLL 缺失/不匹配，或渲染设备初始化失败）",
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
        CancellationToken ct = default,
        PlayOpenOptions? options = null
    )
    {
        await _openGate.WaitAsync(ct);
        try
        {
            // Keep main chain stable: the whole player open/play state machine is executed on UI thread.
            // This avoids sync-blocking UI marshalling (GetResult) which is a common trigger for 0x8001010E.
            await RunOnUiAsync(() => PlayCoreOnUiAsync(site, url, backupUrls, referer, userAgent, ct, options), ct);
        }
        finally
        {
            _openGate.Release();
        }
    }

    public void Stop()
    {
        try
        {
            if (_dq.HasThreadAccess)
            {
                _player.Stop();
            }
            else
            {
                // Don't block. If we're called off-thread (teardown/background), just enqueue best-effort.
                _dq.TryEnqueue(() =>
                {
                    try { _player.Stop(); } catch { }
                });
            }
        }
        catch
        {
            // ignore
        }
    }

    private async Task PlayCoreOnUiAsync(
        string site,
        string url,
        IReadOnlyList<string> backupUrls,
        string? referer,
        string? userAgent,
        CancellationToken ct,
        PlayOpenOptions? options
    )
    {
        if (!_dq.HasThreadAccess)
        {
            throw new InvalidOperationException("PlayCoreOnUiAsync must run on UI thread.");
        }

        _ = site;
        options ??= new PlayOpenOptions();

        // Ensure a clean state before trying URLs.
        try { _player.Stop(); } catch { }
        ApplyHttpHintsUnsafe(referer, userAgent);

        var primary = (url ?? "").Trim();
        var backups = (backupUrls ?? Array.Empty<string>())
            .Select(u => (u ?? "").Trim())
            .Where(u => !string.IsNullOrWhiteSpace(u))
            .ToList();

        if (string.IsNullOrWhiteSpace(primary) && backups.Count == 0)
        {
            throw new InvalidOperationException("empty url");
        }

        var candidates = new List<string>();
        if (!string.IsNullOrWhiteSpace(primary))
        {
            candidates.Add(primary);
        }
        candidates.AddRange(backups);

        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        candidates = candidates.Where(u => seen.Add(u)).ToList();

        Exception? last = null;
        foreach (var u in candidates)
        {
            for (var attempt = 0; attempt <= Math.Max(0, options.RetryPerUrl); attempt++)
            {
                try
                {
                    ct.ThrowIfCancellationRequested();
                    Info?.Invoke(this, $"加载[{attempt + 1}/{Math.Max(0, options.RetryPerUrl) + 1}]：{u}");

                    using var openCts = CancellationTokenSource.CreateLinkedTokenSource(ct);
                    openCts.CancelAfter(options.OpenTimeout);
                    await OpenWithDefaultsOnUiAsync(u, openCts.Token);

                    Info?.Invoke(this, "播放开始");
                    return;
                }
                catch (OperationCanceledException ocex) when (!ct.IsCancellationRequested)
                {
                    last = ocex;
                    Error?.Invoke(this, $"打开超时：{u}");
                }
                catch (Exception ex)
                {
                    last = ex;
                    Error?.Invoke(this, $"尝试播放失败：{ex.Message}");
                }

                if (attempt < Math.Max(0, options.RetryPerUrl))
                {
                    await Task.Delay(options.RetryDelay, ct);
                }
            }
        }

        throw last ?? new Exception("play failed");
    }

    private void ApplyHttpHintsUnsafe(string? referer, string? userAgent)
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

    private async Task OpenWithDefaultsOnUiAsync(string url, CancellationToken ct)
    {
        if (!_dq.HasThreadAccess)
        {
            throw new InvalidOperationException("OpenWithDefaultsOnUiAsync must run on UI thread.");
        }

        // Flyleaf may raise OpenCompleted from a non-UI thread, and `OpenCompletedArgs` can be thread-affine.
        // Always marshal args inspection back to the UI thread to avoid COM marshaling failures (0x8001010E).
        var tcs = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);

        EventHandler<OpenCompletedArgs>? handler = null;
        handler = (_, args) =>
        {
            if (args is null)
            {
                return;
            }

            var ok = _dq.TryEnqueue(() =>
            {
                try
                {
                    if (args.IsSubtitles)
                    {
                        return;
                    }

                    if (args.Success)
                    {
                        tcs.TrySetResult(true);
                    }
                    else
                    {
                        tcs.TrySetException(new Exception(args.Error ?? "open failed"));
                    }
                }
                catch (Exception ex)
                {
                    tcs.TrySetException(ex);
                }
            });

            if (!ok)
            {
                tcs.TrySetException(new InvalidOperationException("failed to enqueue OpenCompleted handler"));
            }
        };

        _player.OpenCompleted += handler;

        CancellationTokenRegistration ctr = default;
        try
        {
            if (ct.CanBeCanceled)
            {
                // Cancellation callback must not touch WinRT/Flyleaf objects.
                ctr = ct.Register(() => tcs.TrySetCanceled(ct));
            }

            _player.OpenAsync(url);
            _player.Play();
            await tcs.Task;
        }
        finally
        {
            ctr.Dispose();
            try
            {
                _player.OpenCompleted -= handler;
            }
            catch
            {
                // ignore
            }
        }
    }

    private Task RunOnUiAsync(Action action, CancellationToken ct = default)
    {
        if (_dq.HasThreadAccess)
        {
            action();
            return Task.CompletedTask;
        }

        var tcs = new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously);
        if (ct.CanBeCanceled)
        {
            ct.Register(() => tcs.TrySetCanceled(ct));
        }

        if (!_dq.TryEnqueue(() =>
        {
            try
            {
                action();
                tcs.TrySetResult(null);
            }
            catch (Exception ex)
            {
                tcs.TrySetException(ex);
            }
        }))
        {
            tcs.TrySetException(new InvalidOperationException("failed to enqueue UI action"));
        }

        return tcs.Task;
    }

    private Task RunOnUiAsync(Func<Task> action, CancellationToken ct = default)
    {
        if (_dq.HasThreadAccess)
        {
            return action();
        }

        var tcs = new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously);
        if (ct.CanBeCanceled)
        {
            ct.Register(() => tcs.TrySetCanceled(ct));
        }

        if (!_dq.TryEnqueue(() =>
        {
            _ = InvokeInnerAsync();
            return;

            async Task InvokeInnerAsync()
            {
                try
                {
                    await action();
                    tcs.TrySetResult(null);
                }
                catch (Exception ex)
                {
                    tcs.TrySetException(ex);
                }
            }
        }))
        {
            tcs.TrySetException(new InvalidOperationException("failed to enqueue UI async action"));
        }

        return tcs.Task;
    }

    public void Dispose()
    {
        Stop();
        try
        {
            _openGate.Dispose();
        }
        catch
        {
            // ignore
        }

        try
        {
            if (_dq.HasThreadAccess)
            {
                _player.Dispose();
            }
            else
            {
                _dq.TryEnqueue(() =>
                {
                    try { _player.Dispose(); } catch { }
                });
            }
        }
        catch
        {
            // ignore
        }
    }
}
