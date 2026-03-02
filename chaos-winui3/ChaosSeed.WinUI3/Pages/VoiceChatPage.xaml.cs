using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using ChaosSeed.WinUI3.Models.Llm;
using ChaosSeed.WinUI3.Models.Voice;
using ChaosSeed.WinUI3.Services;
using ChaosSeed.WinUI3.Services.Audio;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml.Controls;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class VoiceChatPage : Page
{
    private readonly DispatcherQueue _dq = DispatcherQueue.GetForCurrentThread();

    private CancellationTokenSource? _cts;
    private string? _sessionId;
    private ulong _chunks;

    public VoiceChatPage()
    {
        InitializeComponent();

        Loaded += (_, _) => OnLoaded();
        Unloaded += (_, _) => _ = OnUnloadedAsync();
    }

    private void OnLoaded()
    {
        try
        {
            var s = SettingsService.Instance.Current;
            if (string.IsNullOrWhiteSpace(ModelDirBox.Text))
            {
                ModelDirBox.Text = (s.TtsCosyVoicePackDir ?? "").Trim();
            }
            if (string.IsNullOrWhiteSpace(SpkIdBox.Text))
            {
                SpkIdBox.Text = (s.TtsLastSpkId ?? "").Trim();
            }
            if (string.IsNullOrWhiteSpace(InputTextBox.Text))
            {
                InputTextBox.Text = "你好";
            }
        }
        catch (Exception ex)
        {
            AppLog.Exception("VoiceChatPage.OnLoaded", ex);
        }

        try
        {
            DaemonClient.Instance.VoiceChatChunkReceived -= OnVoiceChatChunk;
            DaemonClient.Instance.VoiceChatChunkReceived += OnVoiceChatChunk;
        }
        catch
        {
            // ignore
        }
    }

    private async Task OnUnloadedAsync()
    {
        try
        {
            DaemonClient.Instance.VoiceChatChunkReceived -= OnVoiceChatChunk;
        }
        catch
        {
            // ignore
        }

        await CancelInternalAsync("page_unloaded");
    }

    private void OnVoiceChatChunk(object? sender, VoiceChatChunkNotif msg)
    {
        _ = sender;

        try
        {
            var sid = _sessionId;
            if (string.IsNullOrWhiteSpace(sid))
            {
                return;
            }
            if (!string.Equals((msg.SessionId ?? "").Trim(), sid, StringComparison.Ordinal))
            {
                return;
            }

            if (!string.IsNullOrWhiteSpace(msg.PcmBase64))
            {
                byte[] pcm;
                try
                {
                    pcm = Convert.FromBase64String(msg.PcmBase64);
                }
                catch
                {
                    pcm = Array.Empty<byte>();
                }

                if (pcm.Length > 0)
                {
                    Pcm16StreamPlayerService.Instance.EnqueuePcm16(pcm);
                }
            }

            _chunks = msg.Seq + 1;

            _ = _dq.TryEnqueue(() =>
            {
                StatusText.Text = $"status: streaming (chunks={_chunks})";
            });

            if (msg.IsLast)
            {
                _ = _dq.TryEnqueue(async () => await FinishSessionAsync("done"));
            }
        }
        catch (Exception ex)
        {
            AppLog.Exception("VoiceChatPage.OnVoiceChatChunk", ex);
            _ = _dq.TryEnqueue(async () => await FinishSessionAsync("error"));
        }
    }

    private async void OnStartClicked(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        _ = sender;
        _ = e;

        if (_cts is not null)
        {
            return;
        }

        var text = (InputTextBox.Text ?? "").Trim();
        if (string.IsNullOrWhiteSpace(text))
        {
            StatusText.Text = "status: 输入为空";
            return;
        }

        var modelDir = (ModelDirBox.Text ?? "").Trim();
        var spkId = (SpkIdBox.Text ?? "").Trim();

        // 持久化一些默认输入，便于下次打开页面继续使用。
        try
        {
            SettingsService.Instance.Update(x =>
            {
                x.TtsCosyVoicePackDir = modelDir;
                x.TtsLastSpkId = spkId;
            });
        }
        catch
        {
            // ignore
        }

        StartBtn.IsEnabled = false;
        CancelBtn.IsEnabled = true;
        StatusText.Text = "status: starting...";
        SessionText.Text = "session: (starting)";

        _chunks = 0;
        _sessionId = null;

        _cts = new CancellationTokenSource();
        var ct = _cts.Token;

        try
        {
            // 这里的 TTS 参数只给出一套偏保守的默认值；更细的调参仍建议去 TTS 调试页。
            var p = new VoiceChatStreamStartParams
            {
                ModelDir = modelDir,
                SpkId = spkId,
                ReasoningMode = (ReasoningToggle.IsOn ? "reasoning" : "normal"),
                Messages = new List<ChatMessage> { new ChatMessage { Role = "user", Content = text } },

                PromptText = "<|endofprompt|>",
                PromptStrategy = "inject",
                GuideSep = "。 ",
                Speed = 1.1,
                Seed = 1986,
                Temperature = 1.0,
                TopP = 0.75,
                TopK = 20,
                WinSize = 10,
                TauR = 1.0,
                TextFrontend = true,
                ChunkMs = 100,
            };

            var res = await DaemonClient.Instance.VoiceChatStreamStartAsync(p, ct);
            ct.ThrowIfCancellationRequested();

            _sessionId = (res.SessionId ?? "").Trim();
            if (string.IsNullOrWhiteSpace(_sessionId))
            {
                throw new InvalidOperationException("daemon returned empty sessionId");
            }

            SessionText.Text = $"session: {_sessionId}  sr={res.SampleRate}  ch={res.Channels}  fmt={res.Format}";
            StatusText.Text = "status: connected, waiting chunks...";

            await Pcm16StreamPlayerService.Instance.StartAsync(res.SampleRate, res.Channels, ct);
        }
        catch (OperationCanceledException)
        {
            await FinishSessionAsync("canceled");
        }
        catch (Exception ex)
        {
            AppLog.Exception("VoiceChatPage.Start", ex);
            StatusText.Text = $"status: failed: {ex.Message}";
            await FinishSessionAsync("failed");
        }
    }

    private async void OnCancelClicked(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        _ = sender;
        _ = e;

        await CancelInternalAsync("user_cancel");
    }

    private async Task CancelInternalAsync(string reason)
    {
        _ = reason;

        var sid = _sessionId;
        if (_cts is null && string.IsNullOrWhiteSpace(sid))
        {
            return;
        }

        try
        {
            _cts?.Cancel();
        }
        catch
        {
            // ignore
        }

        try
        {
            if (!string.IsNullOrWhiteSpace(sid))
            {
                await DaemonClient.Instance.VoiceChatStreamCancelAsync(sid);
            }
        }
        catch
        {
            // ignore
        }

        await FinishSessionAsync("canceled");
    }

    private async Task FinishSessionAsync(string reason)
    {
        _ = reason;

        try
        {
            await Pcm16StreamPlayerService.Instance.StopAsync();
        }
        catch
        {
            // ignore
        }

        try
        {
            _cts?.Dispose();
        }
        catch
        {
            // ignore
        }
        _cts = null;
        _sessionId = null;
        _chunks = 0;

        try
        {
            StartBtn.IsEnabled = true;
            CancelBtn.IsEnabled = false;
            SessionText.Text = "session: (none)";
            StatusText.Text = "status: idle";
        }
        catch
        {
            // ignore
        }
    }
}
