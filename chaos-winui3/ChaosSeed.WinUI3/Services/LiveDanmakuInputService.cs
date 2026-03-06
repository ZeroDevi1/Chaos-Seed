namespace ChaosSeed.WinUI3.Services;

public sealed class LiveDanmakuInputService
{
    public static LiveDanmakuInputService Instance { get; } = new();

    private readonly object _gate = new();
    private string _currentInput = string.Empty;

    private LiveDanmakuInputService()
    {
    }

    public event EventHandler<string>? InputChanged;

    public string CurrentInput
    {
        get
        {
            lock (_gate)
            {
                return _currentInput;
            }
        }
    }

    public void Publish(string? input)
    {
        var next = (input ?? string.Empty).Trim();
        string current;
        lock (_gate)
        {
            current = _currentInput;
            if (string.Equals(current, next, StringComparison.Ordinal))
            {
                return;
            }

            _currentInput = next;
        }

        InputChanged?.Invoke(this, next);
    }
}
