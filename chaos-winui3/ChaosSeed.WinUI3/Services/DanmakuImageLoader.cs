using System.Runtime.InteropServices.WindowsRuntime;
using ChaosSeed.WinUI3.Models;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Storage.Streams;

namespace ChaosSeed.WinUI3.Services;

public sealed class DanmakuImageLoader : IDisposable
{
    private readonly SemaphoreSlim _sem;
    private readonly Microsoft.UI.Dispatching.DispatcherQueue _dq;

    public DanmakuImageLoader(Microsoft.UI.Dispatching.DispatcherQueue dq, int maxConcurrency = 4)
    {
        _dq = dq;
        _sem = new SemaphoreSlim(Math.Max(1, maxConcurrency), Math.Max(1, maxConcurrency));
    }

    public async Task TryLoadEmoteAsync(string sessionId, DanmakuRowVm row, string url, CancellationToken ct)
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid))
        {
            return;
        }
        if (!string.Equals(DanmakuService.Instance.CurrentSessionId, sid, StringComparison.Ordinal))
        {
            return;
        }

        await _sem.WaitAsync(ct);
        try
        {
            if (!string.Equals(DanmakuService.Instance.CurrentSessionId, sid, StringComparison.Ordinal))
            {
                return;
            }

            var res = await DanmakuService.Instance.FetchImageAsync(sid, url, ct);
            if (string.IsNullOrWhiteSpace(res.Base64))
            {
                return;
            }

            var bytes = Convert.FromBase64String(res.Base64);
            using var ms = new InMemoryRandomAccessStream();
            await ms.WriteAsync(bytes.AsBuffer());
            ms.Seek(0);

            await RunOnUiAsync(async () =>
            {
                if (!string.Equals(DanmakuService.Instance.CurrentSessionId, sid, StringComparison.Ordinal))
                {
                    return;
                }

                var bmp = new BitmapImage();
                ms.Seek(0);
                await bmp.SetSourceAsync(ms);
                row.Emote = bmp;
            });
        }
        catch
        {
            // ignore image load failures
        }
        finally
        {
            _sem.Release();
        }
    }

    public void Dispose()
    {
        _sem.Dispose();
    }

    private Task RunOnUiAsync(Func<Task> action)
    {
        var tcs = new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously);
        if (!_dq.TryEnqueue(async () =>
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
            }))
        {
            tcs.TrySetException(new InvalidOperationException("dispatcher queue unavailable"));
        }
        return tcs.Task;
    }
}

