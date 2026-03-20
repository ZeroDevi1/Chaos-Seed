using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;

namespace ChaosSeed.WinUI3.Cli;

/// <summary>
/// FFI 后端实现
/// </summary>
public class FfiCliBackend : ICliBackend
{
    private const string DllName = "chaos_ffi";
    private IntPtr? _danmakuHandle;

    #region FFI Imports

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr chaos_ffi_last_error_json();

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    private static extern IntPtr chaos_livestream_decode_manifest_json(string input_utf8, byte drop_inaccessible_high_qualities);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    private static extern IntPtr chaos_livestream_resolve_variant2_json(string site_utf8, string room_id_utf8, string variant_id_utf8);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    private static extern IntPtr chaos_danmaku_connect(string input_utf8);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr chaos_danmaku_poll_json(IntPtr handle, uint max_events);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    private static extern int chaos_danmaku_disconnect(IntPtr handle);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    private static extern void chaos_ffi_string_free(IntPtr s);

    #endregion

    public Task InitializeAsync(CancellationToken ct = default)
    {
        // FFI 不需要额外初始化
        return Task.CompletedTask;
    }

    public async Task<LiveManifest?> DecodeManifestAsync(string input, bool dropInaccessibleHighQualities = true, CancellationToken ct = default)
    {
        return await Task.Run(() =>
        {
            var ptr = chaos_livestream_decode_manifest_json(input, dropInaccessibleHighQualities ? (byte)1 : (byte)0);
            if (ptr == IntPtr.Zero)
            {
                var errPtr = chaos_ffi_last_error_json();
                var err = errPtr != IntPtr.Zero ? PtrToString(errPtr) : "Unknown error";
                throw new InvalidOperationException($"解析直播源失败: {err}");
            }

            var json = PtrToString(ptr);
            chaos_ffi_string_free(ptr);

            return JsonSerializer.Deserialize<LiveManifest>(json);
        }, ct);
    }

    public async Task<StreamVariant?> ResolveVariantAsync(string site, string roomId, string variantId, CancellationToken ct = default)
    {
        return await Task.Run(() =>
        {
            var ptr = chaos_livestream_resolve_variant2_json(site, roomId, variantId);
            if (ptr == IntPtr.Zero)
            {
                var errPtr = chaos_ffi_last_error_json();
                var err = errPtr != IntPtr.Zero ? PtrToString(errPtr) : "Unknown error";
                throw new InvalidOperationException($"解析清晰度失败: {err}");
            }

            var json = PtrToString(ptr);
            chaos_ffi_string_free(ptr);

            return JsonSerializer.Deserialize<StreamVariant>(json);
        }, ct);
    }

    public async Task<IDanmakuSession?> ConnectDanmakuAsync(string input, CancellationToken ct = default)
    {
        return await Task.Run(() =>
        {
            var handle = chaos_danmaku_connect(input);
            if (handle == IntPtr.Zero)
            {
                var errPtr = chaos_ffi_last_error_json();
                var err = errPtr != IntPtr.Zero ? PtrToString(errPtr) : "Unknown error";
                throw new InvalidOperationException($"连接弹幕失败: {err}");
            }

            _danmakuHandle = handle;
            return new FfiDanmakuSession(handle);
        }, ct);
    }

    public ValueTask DisposeAsync()
    {
        if (_danmakuHandle.HasValue && _danmakuHandle.Value != IntPtr.Zero)
        {
            chaos_danmaku_disconnect(_danmakuHandle.Value);
            _danmakuHandle = null;
        }
        return ValueTask.CompletedTask;
    }

    private static string PtrToString(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero) return string.Empty;

        // 尝试读取 UTF-8 字符串
        var len = 0;
        while (Marshal.ReadByte(ptr, len) != 0) len++;

        var bytes = new byte[len];
        Marshal.Copy(ptr, bytes, 0, len);
        return Encoding.UTF8.GetString(bytes);
    }
}

/// <summary>
/// FFI 弹幕会话实现
/// </summary>
public class FfiDanmakuSession : IDanmakuSession
{
    private readonly IntPtr _handle;

    [DllImport("chaos_ffi", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr chaos_danmaku_poll_json(IntPtr handle, uint max_events);

    [DllImport("chaos_ffi", CallingConvention = CallingConvention.Cdecl)]
    private static extern int chaos_danmaku_disconnect(IntPtr handle);

    [DllImport("chaos_ffi", CallingConvention = CallingConvention.Cdecl)]
    private static extern void chaos_ffi_string_free(IntPtr s);

    public FfiDanmakuSession(IntPtr handle)
    {
        _handle = handle;
    }

    public Task<List<DanmakuEvent>> PollAsync(int maxEvents = 50, CancellationToken ct = default)
    {
        return Task.Run(() =>
        {
            var ptr = chaos_danmaku_poll_json(_handle, (uint)maxEvents);
            if (ptr == IntPtr.Zero)
            {
                return new List<DanmakuEvent>();
            }

            var json = PtrToString(ptr);
            chaos_ffi_string_free(ptr);

            return JsonSerializer.Deserialize<List<DanmakuEvent>>(json) ?? new List<DanmakuEvent>();
        }, ct);
    }

    public ValueTask DisposeAsync()
    {
        chaos_danmaku_disconnect(_handle);
        return ValueTask.CompletedTask;
    }

    private static string PtrToString(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero) return string.Empty;

        var len = 0;
        while (Marshal.ReadByte(ptr, len) != 0) len++;

        var bytes = new byte[len];
        Marshal.Copy(ptr, bytes, 0, len);
        return Encoding.UTF8.GetString(bytes);
    }
}
