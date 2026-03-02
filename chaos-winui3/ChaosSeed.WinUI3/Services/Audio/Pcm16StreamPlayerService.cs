using System;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;
using global::Windows.Media;
using global::Windows.Media.Audio;
using global::Windows.Media.MediaProperties;
using global::Windows.Media.Render;
using WinRT;

namespace ChaosSeed.WinUI3.Services.Audio;

/// <summary>
/// 实时 PCM16LE 播放器（用于 voice.chat.chunk：边收边播）。
/// </summary>
public sealed class Pcm16StreamPlayerService : IDisposable
{
    public static Pcm16StreamPlayerService Instance => _instance.Value;
    private static readonly Lazy<Pcm16StreamPlayerService> _instance =
        new(() => new Pcm16StreamPlayerService());

    private readonly object _gate = new();
    private readonly SemaphoreSlim _startStopLock = new(1, 1);

    private AudioGraph? _graph;
    private AudioDeviceOutputNode? _out;
    private AudioFrameInputNode? _in;

    public bool IsRunning { get; private set; }
    public uint SampleRate { get; private set; }
    public ushort Channels { get; private set; }

    private Pcm16StreamPlayerService() { }

    public async Task StartAsync(uint sampleRate, ushort channels, CancellationToken ct = default)
    {
        if (sampleRate == 0) throw new ArgumentException("sampleRate must be > 0", nameof(sampleRate));
        if (channels == 0) throw new ArgumentException("channels must be > 0", nameof(channels));

        await _startStopLock.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();

            // 已在同参数运行时，不重复创建。
            if (IsRunning && _graph is not null && _in is not null && SampleRate == sampleRate && Channels == channels)
            {
                return;
            }

            StopInternal();

            var settings = new AudioGraphSettings(AudioRenderCategory.Speech)
            {
                DesiredSamplesPerQuantum = 480, // 约 20ms @ 24k；不是严格要求
            };

            var graphRes = await AudioGraph.CreateAsync(settings);
            if (graphRes.Status != AudioGraphCreationStatus.Success || graphRes.Graph is null)
            {
                throw new InvalidOperationException($"AudioGraph create failed: {graphRes.Status}");
            }

            var graph = graphRes.Graph;
            var outRes = await graph.CreateDeviceOutputNodeAsync();
            if (outRes.Status != AudioDeviceNodeCreationStatus.Success || outRes.DeviceOutputNode is null)
            {
                graph.Dispose();
                throw new InvalidOperationException($"Audio output node create failed: {outRes.Status}");
            }

            var props = AudioEncodingProperties.CreatePcm(sampleRate, channels, 16);
            var input = graph.CreateFrameInputNode(props);
            input.AddOutgoingConnection(outRes.DeviceOutputNode);

            input.Start();
            graph.Start();

            lock (_gate)
            {
                _graph = graph;
                _out = outRes.DeviceOutputNode;
                _in = input;
                SampleRate = sampleRate;
                Channels = channels;
                IsRunning = true;
            }
        }
        finally
        {
            _startStopLock.Release();
        }
    }

    public void EnqueuePcm16(byte[] pcm16le)
    {
        if (pcm16le is null || pcm16le.Length == 0)
        {
            return;
        }
        // PCM16LE：必须按 2 字节对齐。
        if ((pcm16le.Length & 1) != 0)
        {
            return;
        }

        AudioFrameInputNode? node;
        lock (_gate)
        {
            node = _in;
        }
        if (node is null)
        {
            return;
        }

        try
        {
            // AudioFrame payload size is bytes.
            var frame = new AudioFrame((uint)pcm16le.Length);
            using var buffer = frame.LockBuffer(AudioBufferAccessMode.Write);
            buffer.Length = (uint)pcm16le.Length;
            using var reference = buffer.CreateReference();

            unsafe
            {
                var access = reference.As<IMemoryBufferByteAccess>();
                access.GetBuffer(out var data, out var cap);
                var n = (int)Math.Min((uint)pcm16le.Length, cap);
                Marshal.Copy(pcm16le, 0, (IntPtr)data, n);
            }

            node.AddFrame(frame);
        }
        catch
        {
            // best-effort：实时播放链路不因单个 chunk 异常而中断
        }
    }

    public async Task StopAsync(CancellationToken ct = default)
    {
        await _startStopLock.WaitAsync(ct);
        try
        {
            StopInternal();
        }
        finally
        {
            _startStopLock.Release();
        }
    }

    private void StopInternal()
    {
        lock (_gate)
        {
            try { _in?.Stop(); } catch { }
            try { _graph?.Stop(); } catch { }

            try { _in?.Dispose(); } catch { }
            try { _out?.Dispose(); } catch { }
            try { _graph?.Dispose(); } catch { }

            _in = null;
            _out = null;
            _graph = null;
            IsRunning = false;
        }
    }

    public void Dispose()
    {
        StopInternal();
        try { _startStopLock.Dispose(); } catch { }
    }

    [ComImport]
    [Guid("5B0D3235-4DBA-4D44-865E-8F1D0E4FD04D")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private unsafe interface IMemoryBufferByteAccess
    {
        void GetBuffer(out byte* buffer, out uint capacity);
    }
}
