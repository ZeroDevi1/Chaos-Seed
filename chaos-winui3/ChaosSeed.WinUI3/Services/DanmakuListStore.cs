using System.Collections.ObjectModel;
using ChaosSeed.WinUI3.Models;

namespace ChaosSeed.WinUI3.Services;

public sealed class DanmakuListStore : IDisposable
{
    private readonly ObservableCollection<DanmakuRowVm> _rows;
    private readonly Action<DanmakuMessage, DanmakuRowVm>? _rowCreated;

    private readonly object _gate = new();
    private readonly Queue<DanmakuMessage> _pending = new();
    private readonly Dictionary<string, long> _recent = new();

    private readonly Microsoft.UI.Dispatching.DispatcherQueueTimer _timer;

    private readonly int _maxRows;
    private readonly int _maxFlushPerTick;
    private const int DedupeWindowMs = 80;

    public DanmakuListStore(
        Microsoft.UI.Dispatching.DispatcherQueue dq,
        ObservableCollection<DanmakuRowVm> rows,
        Action<DanmakuMessage, DanmakuRowVm>? rowCreated = null,
        int maxRows = 400,
        int maxFlushPerTick = 30
    )
    {
        _rows = rows;
        _rowCreated = rowCreated;
        _maxRows = Math.Max(20, maxRows);
        _maxFlushPerTick = Math.Max(1, maxFlushPerTick);

        _timer = dq.CreateTimer();
        _timer.Interval = TimeSpan.FromMilliseconds(16);
        _timer.IsRepeating = true;
        _timer.Tick += (_, _) => FlushTick();
        _timer.Start();
    }

    public void Enqueue(DanmakuMessage msg)
    {
        if (msg is null)
        {
            return;
        }

        var user = (msg.User ?? "").Trim();
        var text = (msg.Text ?? "").Trim();
        var imageUrl = (msg.ImageUrl ?? "").Trim();

        if (user.Length == 0 && text.Length == 0 && imageUrl.Length == 0)
        {
            return;
        }

        var now = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        var key = $"{user}\n{text}\n{imageUrl}";

        lock (_gate)
        {
            if (_recent.TryGetValue(key, out var last) && now - last < DedupeWindowMs)
            {
                return;
            }
            _recent[key] = now;

            _pending.Enqueue(msg);

            // Overload protection: keep the UI responsive.
            if (_pending.Count > 1200)
            {
                while (_pending.Count > 200)
                {
                    _pending.Dequeue();
                }
            }

            // Prevent `_recent` from growing unbounded.
            if (_recent.Count > 2000)
            {
                _recent.Clear();
            }
        }
    }

    public void Dispose()
    {
        try
        {
            _timer.Stop();
        }
        catch
        {
            // ignore
        }

        lock (_gate)
        {
            _pending.Clear();
            _recent.Clear();
        }
    }

    private void FlushTick()
    {
        List<DanmakuMessage>? batch = null;
        lock (_gate)
        {
            if (_pending.Count == 0)
            {
                return;
            }

            var n = Math.Min(_maxFlushPerTick, _pending.Count);
            batch = new List<DanmakuMessage>(n);
            for (var i = 0; i < n; i++)
            {
                batch.Add(_pending.Dequeue());
            }
        }

        if (batch is null || batch.Count == 0)
        {
            return;
        }

        foreach (var msg in batch)
        {
            var user = (msg.User ?? "").Trim();
            if (user.Length == 0)
            {
                user = "??";
            }

            var text = (msg.Text ?? "").Trim();
            if (text.Length == 0 && !string.IsNullOrWhiteSpace(msg.ImageUrl))
            {
                text = "[图片]";
            }

            var row = new DanmakuRowVm(user, text);
            _rows.Insert(0, row);
            _rowCreated?.Invoke(msg, row);

            while (_rows.Count > _maxRows)
            {
                _rows.RemoveAt(_rows.Count - 1);
            }
        }
    }
}

