using System;
using System.Collections.Generic;
using System.Linq;
using ChaosSeed.WinUI3.Models.Music;

namespace ChaosSeed.WinUI3.Services;

public enum MusicLoopMode
{
    Single = 0,
    All = 1,
    Off = 2,
}

public sealed class MusicQueueItem
{
    public MusicQueueItem(MusicTrack track, string requestedQualityId)
    {
        Track = track ?? throw new ArgumentNullException(nameof(track));
        RequestedQualityId = (requestedQualityId ?? "").Trim();
    }

    public MusicTrack Track { get; }
    public string RequestedQualityId { get; }

    public string Key => BuildKey(Track);

    public static string BuildKey(MusicTrack track)
    {
        var svc = (track?.Service ?? "").Trim();
        var id = (track?.Id ?? "").Trim();
        return $"{svc}:{id}";
    }
}

public sealed class MusicQueueState
{
    private readonly List<MusicQueueItem> _items = new();

    public IReadOnlyList<MusicQueueItem> Items => _items;

    public int CurrentIndex { get; private set; } = -1;

    public MusicQueueItem? CurrentItem =>
        (CurrentIndex >= 0 && CurrentIndex < _items.Count) ? _items[CurrentIndex] : null;

    public int IndexOfKey(string key)
    {
        if (string.IsNullOrWhiteSpace(key))
        {
            return -1;
        }

        for (var i = 0; i < _items.Count; i++)
        {
            if (string.Equals(_items[i].Key, key, StringComparison.Ordinal))
            {
                return i;
            }
        }

        return -1;
    }

    public bool Enqueue(MusicQueueItem item, out int index)
    {
        if (item is null) throw new ArgumentNullException(nameof(item));

        var existing = IndexOfKey(item.Key);
        if (existing >= 0)
        {
            index = existing;
            return false;
        }

        _items.Add(item);
        index = _items.Count - 1;
        return true;
    }

    public bool PlayNow(MusicQueueItem item, out int index, out bool inserted)
    {
        if (item is null) throw new ArgumentNullException(nameof(item));

        var existing = IndexOfKey(item.Key);
        if (existing >= 0)
        {
            CurrentIndex = existing;
            index = existing;
            inserted = false;
            return false;
        }

        var insertAt = CurrentIndex >= 0 && CurrentIndex < _items.Count
            ? CurrentIndex + 1
            : _items.Count;

        if (insertAt < 0) insertAt = 0;
        if (insertAt > _items.Count) insertAt = _items.Count;

        _items.Insert(insertAt, item);
        CurrentIndex = insertAt;
        index = insertAt;
        inserted = true;
        return true;
    }

    public bool RemoveAt(int index, out bool removedCurrent)
    {
        removedCurrent = false;
        if (index < 0 || index >= _items.Count)
        {
            return false;
        }

        removedCurrent = index == CurrentIndex;
        _items.RemoveAt(index);

        if (_items.Count == 0)
        {
            CurrentIndex = -1;
            return true;
        }

        if (CurrentIndex > index)
        {
            CurrentIndex--;
            return true;
        }

        if (removedCurrent)
        {
            if (index >= _items.Count)
            {
                CurrentIndex = _items.Count - 1;
            }
        }

        return true;
    }

    public void Clear()
    {
        _items.Clear();
        CurrentIndex = -1;
    }

    public bool TrySetCurrentIndex(int index)
    {
        if (index < 0 || index >= _items.Count)
        {
            return false;
        }
        CurrentIndex = index;
        return true;
    }

    public bool TryGetNextIndex(MusicLoopMode loopMode, out int nextIndex)
    {
        nextIndex = -1;
        if (_items.Count == 0)
        {
            return false;
        }

        var cur = CurrentIndex;
        if (cur < 0 || cur >= _items.Count)
        {
            nextIndex = 0;
            return true;
        }

        var cand = cur + 1;
        if (cand < _items.Count)
        {
            nextIndex = cand;
            return true;
        }

        if (loopMode == MusicLoopMode.All)
        {
            nextIndex = 0;
            return true;
        }

        return false;
    }

    public bool TryGetPrevIndex(MusicLoopMode loopMode, out int prevIndex)
    {
        prevIndex = -1;
        if (_items.Count == 0)
        {
            return false;
        }

        var cur = CurrentIndex;
        if (cur < 0 || cur >= _items.Count)
        {
            prevIndex = 0;
            return true;
        }

        var cand = cur - 1;
        if (cand >= 0)
        {
            prevIndex = cand;
            return true;
        }

        if (loopMode == MusicLoopMode.All)
        {
            prevIndex = _items.Count - 1;
            return true;
        }

        return false;
    }
}

