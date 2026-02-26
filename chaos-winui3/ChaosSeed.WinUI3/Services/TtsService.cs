using ChaosSeed.WinUI3.Models.Tts;

namespace ChaosSeed.WinUI3.Services;

public sealed class TtsService
{
    private readonly DaemonClient _daemon;

    public TtsService(DaemonClient daemon)
    {
        _daemon = daemon ?? throw new ArgumentNullException(nameof(daemon));
    }

    public async Task<string> SynthesizeSftToWavFileAsync(
        TtsSftStartParams p,
        TimeSpan? pollInterval = null,
        CancellationToken ct = default
    )
    {
        if (p is null) throw new ArgumentNullException(nameof(p));

        var start = await _daemon.TtsSftStartAsync(p, ct);
        var sid = (start.SessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid))
        {
            throw new InvalidOperationException("tts.sft.start returned empty sessionId");
        }

        var interval = pollInterval ?? TimeSpan.FromMilliseconds(250);
        while (true)
        {
            ct.ThrowIfCancellationRequested();
            var st = await _daemon.TtsSftStatusAsync(sid, ct);
            if (!st.Done)
            {
                await Task.Delay(interval, ct);
                continue;
            }

            if (!string.Equals(st.State, "done", StringComparison.OrdinalIgnoreCase))
            {
                var err = (st.Error ?? "").Trim();
                if (string.IsNullOrWhiteSpace(err))
                {
                    err = $"tts job finished in state={st.State}";
                }
                throw new InvalidOperationException(err);
            }

            if (st.Result is null || string.IsNullOrWhiteSpace(st.Result.WavBase64))
            {
                throw new InvalidOperationException("tts.sft.status returned done but result.wavBase64 is empty");
            }

            byte[] wav;
            try
            {
                wav = Convert.FromBase64String(st.Result.WavBase64);
            }
            catch (FormatException ex)
            {
                throw new InvalidOperationException("invalid base64 wav payload", ex);
            }

            var root = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "ChaosSeed.WinUI3",
                "tts"
            );
            Directory.CreateDirectory(root);

            var path = Path.Combine(root, $"{sid}.wav");
            await File.WriteAllBytesAsync(path, wav, ct);
            return path;
        }
    }
}
