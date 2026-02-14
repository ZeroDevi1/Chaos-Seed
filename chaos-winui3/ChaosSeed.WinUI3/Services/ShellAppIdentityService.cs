using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;

namespace ChaosSeed.WinUI3.Services;

public static class ShellAppIdentityService
{
    // For unpackaged WinUI3 desktop apps, SMTC/GSMTC may show "Unknown app" unless the process has an
    // explicit AppUserModelID that maps to a Start Menu shortcut (name/icon).

    private const string DefaultAppId = "ChaosSeed";

    public static void TryEnsureChaosSeedIdentity()
    {
        try
        {
            var exePath = Environment.ProcessPath;
            if (string.IsNullOrWhiteSpace(exePath) || !File.Exists(exePath))
            {
                return;
            }

            var iconPath = Path.Combine(AppContext.BaseDirectory, "Assets", "icon.ico");
            if (!File.Exists(iconPath))
            {
                // Fallback to exe icon (ApplicationIcon) if asset isn't copied.
                iconPath = exePath;
            }

            TryCreateOrUpdateStartMenuShortcut(DefaultAppId, "ChaosSeed", exePath, iconPath);
            TrySetCurrentProcessExplicitAppUserModelId(DefaultAppId);
        }
        catch
        {
            // best-effort
        }
    }

    private static void TrySetCurrentProcessExplicitAppUserModelId(string appId)
    {
        try
        {
            _ = SetCurrentProcessExplicitAppUserModelID(appId);
        }
        catch
        {
            // ignore
        }
    }

    private static void TryCreateOrUpdateStartMenuShortcut(
        string appId,
        string displayName,
        string exePath,
        string iconPath
    )
    {
        var programsDir = Environment.GetFolderPath(Environment.SpecialFolder.Programs);
        if (string.IsNullOrWhiteSpace(programsDir))
        {
            return;
        }

        Directory.CreateDirectory(programsDir);

        var shortcutPath = Path.Combine(programsDir, $"{displayName}.lnk");
        var workDir = AppContext.BaseDirectory.TrimEnd(Path.DirectorySeparatorChar);

        // Always overwrite: avoids stale links when running from a new build output folder.
        var link = (IShellLinkW)new ShellLink();
        try
        {
            link.SetPath(exePath);
            link.SetArguments("");
            link.SetWorkingDirectory(workDir);
            link.SetIconLocation(iconPath, 0);
            link.SetDescription(displayName);

            var store = (IPropertyStore)link;
            try
            {
                var pvAppId = PropVariant.FromString(appId);
                var pvCmd = PropVariant.FromString($"\"{exePath}\"");
                var pvName = PropVariant.FromString(displayName);
                var pvIcon = PropVariant.FromString($"{iconPath},0");

                try
                {
                    // C# does not allow passing `readonly` fields or `using var` locals by ref.
                    var keyAppId = PKEY_AppUserModel_ID;
                    var keyCmd = PKEY_AppUserModel_RelaunchCommand;
                    var keyName = PKEY_AppUserModel_RelaunchDisplayNameResource;
                    var keyIcon = PKEY_AppUserModel_RelaunchIconResource;

                    store.SetValue(ref keyAppId, ref pvAppId);
                    store.SetValue(ref keyCmd, ref pvCmd);
                    store.SetValue(ref keyName, ref pvName);
                    store.SetValue(ref keyIcon, ref pvIcon);
                }
                finally
                {
                    pvIcon.Dispose();
                    pvName.Dispose();
                    pvCmd.Dispose();
                    pvAppId.Dispose();
                }
                store.Commit();
            }
            finally
            {
                Marshal.FinalReleaseComObject(store);
            }

            var persist = (IPersistFile)link;
            try
            {
                persist.Save(shortcutPath, true);
            }
            finally
            {
                Marshal.FinalReleaseComObject(persist);
            }
        }
        finally
        {
            Marshal.FinalReleaseComObject(link);
        }
    }

    // --- Win32/COM interop ---

    [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
    private static extern int SetCurrentProcessExplicitAppUserModelID(string appID);

    [ComImport]
    [Guid("00021401-0000-0000-C000-000000000046")]
    private class ShellLink
    {
    }

    [ComImport]
    [Guid("000214F9-0000-0000-C000-000000000046")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IShellLinkW
    {
        void GetPath(
            [Out, MarshalAs(UnmanagedType.LPWStr)] System.Text.StringBuilder pszFile,
            int cchMaxPath,
            IntPtr pfd,
            uint fFlags
        );

        void GetIDList(out IntPtr ppidl);
        void SetIDList(IntPtr pidl);

        void GetDescription([Out, MarshalAs(UnmanagedType.LPWStr)] System.Text.StringBuilder pszName, int cchMaxName);
        void SetDescription([MarshalAs(UnmanagedType.LPWStr)] string pszName);

        void GetWorkingDirectory(
            [Out, MarshalAs(UnmanagedType.LPWStr)] System.Text.StringBuilder pszDir,
            int cchMaxPath
        );

        void SetWorkingDirectory([MarshalAs(UnmanagedType.LPWStr)] string pszDir);

        void GetArguments([Out, MarshalAs(UnmanagedType.LPWStr)] System.Text.StringBuilder pszArgs, int cchMaxPath);
        void SetArguments([MarshalAs(UnmanagedType.LPWStr)] string pszArgs);

        void GetHotkey(out short pwHotkey);
        void SetHotkey(short wHotkey);

        void GetShowCmd(out int piShowCmd);
        void SetShowCmd(int iShowCmd);

        void GetIconLocation(
            [Out, MarshalAs(UnmanagedType.LPWStr)] System.Text.StringBuilder pszIconPath,
            int cchIconPath,
            out int piIcon
        );

        void SetIconLocation([MarshalAs(UnmanagedType.LPWStr)] string pszIconPath, int iIcon);

        void SetRelativePath([MarshalAs(UnmanagedType.LPWStr)] string pszPathRel, uint dwReserved);
        void Resolve(IntPtr hwnd, uint fFlags);

        void SetPath([MarshalAs(UnmanagedType.LPWStr)] string pszFile);
    }

    [StructLayout(LayoutKind.Sequential, Pack = 4)]
    private struct PropertyKey
    {
        public Guid fmtid;
        public uint pid;
    }

    // https://learn.microsoft.com/windows/win32/properties/props-system-appusermodel-id
    private static readonly PropertyKey PKEY_AppUserModel_ID = new()
    {
        fmtid = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"),
        pid = 5,
    };

    private static readonly PropertyKey PKEY_AppUserModel_RelaunchCommand = new()
    {
        fmtid = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"),
        pid = 2,
    };

    private static readonly PropertyKey PKEY_AppUserModel_RelaunchDisplayNameResource = new()
    {
        fmtid = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"),
        pid = 4,
    };

    private static readonly PropertyKey PKEY_AppUserModel_RelaunchIconResource = new()
    {
        fmtid = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"),
        pid = 3,
    };

    [ComImport]
    [Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IPropertyStore
    {
        uint GetCount(out uint cProps);
        uint GetAt(uint iProp, out PropertyKey pkey);
        uint GetValue(ref PropertyKey key, out PropVariant pv);
        uint SetValue(ref PropertyKey key, ref PropVariant pv);
        uint Commit();
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct PropVariant : IDisposable
    {
        private ushort vt;
        private ushort wReserved1;
        private ushort wReserved2;
        private ushort wReserved3;
        private IntPtr p;
        private int p2;

        public static PropVariant FromString(string value)
        {
            var pv = new PropVariant
            {
                vt = (ushort)VarEnum.VT_LPWSTR,
                p = Marshal.StringToCoTaskMemUni(value ?? ""),
                p2 = 0,
            };
            return pv;
        }

        public void Dispose()
        {
            try
            {
                if (vt == (ushort)VarEnum.VT_LPWSTR && p != IntPtr.Zero)
                {
                    Marshal.FreeCoTaskMem(p);
                }
            }
            catch
            {
                // ignore
            }
            vt = 0;
            p = IntPtr.Zero;
            p2 = 0;
        }
    }
}
