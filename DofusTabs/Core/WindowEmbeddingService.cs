using System;
using System.Runtime.InteropServices;

namespace DofusTabs.Core
{
    public sealed class WindowEmbeddingService : IDisposable
    {
    private static readonly object RecoveryLock = new object();

        private const int GWL_STYLE = -16;
        private const int GWL_EXSTYLE = -20;

        private const long WS_CHILD = 0x40000000L;
        private const long WS_POPUP = unchecked((long)0x80000000);
        private const long WS_CAPTION = 0x00C00000L;
        private const long WS_THICKFRAME = 0x00040000L;
        private const long WS_MINIMIZEBOX = 0x00020000L;
        private const long WS_MAXIMIZEBOX = 0x00010000L;
        private const long WS_SYSMENU = 0x00080000L;

        private const long WS_EX_DLGMODALFRAME = 0x00000001L;
        private const long WS_EX_CLIENTEDGE = 0x00000200L;
        private const long WS_EX_STATICEDGE = 0x00020000L;

        private const uint SWP_NOMOVE = 0x0002;
        private const uint SWP_NOSIZE = 0x0001;
        private const uint SWP_NOZORDER = 0x0004;
        private const uint SWP_NOACTIVATE = 0x0010;
        private const uint SWP_NOOWNERZORDER = 0x0200;
        private const uint SWP_FRAMECHANGED = 0x0020;
        private const uint SWP_SHOWWINDOW = 0x0040;

        private const int SW_RESTORE = 9;
        private const int SW_SHOW = 5;

        private EmbeddedWindowState? _embeddedWindow;

        public bool TryEmbed(WindowInfo windowInfo, IntPtr hostHandle, int hostWidth, int hostHeight)
        {
            if (windowInfo.Handle == IntPtr.Zero || hostHandle == IntPtr.Zero)
            {
                return false;
            }

            if (!IsWindow(windowInfo.Handle))
            {
                return false;
            }

            if (_embeddedWindow != null && _embeddedWindow.Handle == windowInfo.Handle)
            {
                ResizeEmbeddedWindow(hostWidth, hostHeight);
                return true;
            }

            RestoreEmbeddedWindow();

            if (IsIconic(windowInfo.Handle))
            {
                ShowWindow(windowInfo.Handle, SW_RESTORE);
            }

            var state = new EmbeddedWindowState
            {
                Handle = windowInfo.Handle,
                OriginalParent = GetParent(windowInfo.Handle),
                OriginalStyle = GetWindowLongPtrSafe(windowInfo.Handle, GWL_STYLE),
                OriginalExStyle = GetWindowLongPtrSafe(windowInfo.Handle, GWL_EXSTYLE)
            };

            if (SetParent(windowInfo.Handle, hostHandle) == IntPtr.Zero && Marshal.GetLastWin32Error() != 0)
            {
                return false;
            }

            long updatedStyle = state.OriginalStyle.ToInt64();
            updatedStyle |= WS_CHILD;
            updatedStyle &= ~WS_POPUP;
            updatedStyle &= ~WS_CAPTION;
            updatedStyle &= ~WS_THICKFRAME;
            updatedStyle &= ~WS_MINIMIZEBOX;
            updatedStyle &= ~WS_MAXIMIZEBOX;
            updatedStyle &= ~WS_SYSMENU;

            long updatedExStyle = state.OriginalExStyle.ToInt64();
            updatedExStyle &= ~WS_EX_DLGMODALFRAME;
            updatedExStyle &= ~WS_EX_CLIENTEDGE;
            updatedExStyle &= ~WS_EX_STATICEDGE;

            SetWindowLongPtrSafe(windowInfo.Handle, GWL_STYLE, new IntPtr(updatedStyle));
            SetWindowLongPtrSafe(windowInfo.Handle, GWL_EXSTYLE, new IntPtr(updatedExStyle));

            _embeddedWindow = state;

            SetWindowPos(
                _embeddedWindow.Handle,
                IntPtr.Zero,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED);

            ResizeEmbeddedWindow(hostWidth, hostHeight);
            ShowWindow(windowInfo.Handle, SW_SHOW);

            return true;
        }

        public void ResizeEmbeddedWindow(int hostWidth, int hostHeight)
        {
            if (_embeddedWindow == null)
            {
                return;
            }

            if (!IsWindow(_embeddedWindow.Handle))
            {
                _embeddedWindow = null;
                return;
            }

            int width = Math.Max(1, hostWidth);
            int height = Math.Max(1, hostHeight);

            // Evitar reposicionamientos redundantes que provocan parpadeo.
            if (_embeddedWindow.LastWidth == width && _embeddedWindow.LastHeight == height)
            {
                return;
            }

            // Durante layout transitorio, no forzar tamaño casi nulo.
            if (width < 8 || height < 8)
            {
                return;
            }

            SetWindowPos(
                _embeddedWindow.Handle,
                IntPtr.Zero,
                0,
                0,
                width,
                height,
                SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE);

            _embeddedWindow.LastWidth = width;
            _embeddedWindow.LastHeight = height;
        }

        public void RestoreEmbeddedWindow()
        {
            if (_embeddedWindow == null)
            {
                return;
            }

            if (!IsWindow(_embeddedWindow.Handle))
            {
                _embeddedWindow = null;
                return;
            }

            SetParent(_embeddedWindow.Handle, _embeddedWindow.OriginalParent);
            SetWindowLongPtrSafe(_embeddedWindow.Handle, GWL_STYLE, _embeddedWindow.OriginalStyle);
            SetWindowLongPtrSafe(_embeddedWindow.Handle, GWL_EXSTYLE, _embeddedWindow.OriginalExStyle);

            SetWindowPos(
                _embeddedWindow.Handle,
                IntPtr.Zero,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED);

            ShowWindow(_embeddedWindow.Handle, SW_SHOW);
            _embeddedWindow = null;
        }

        public void Dispose()
        {
            RestoreEmbeddedWindow();
        }

        public static void RecoverAllEmbeddedWindows()
        {
            lock (RecoveryLock)
            {
                EnumWindows((hWnd, _) =>
                {
                    if (!IsWindow(hWnd))
                    {
                        return true;
                    }

                    var parent = GetParent(hWnd);
                    if (parent == IntPtr.Zero)
                    {
                        return true;
                    }

                    uint processId;
                    GetWindowThreadProcessId(hWnd, out processId);
                    if (processId == 0)
                    {
                        return true;
                    }

                    string processName;
                    try
                    {
                        processName = System.Diagnostics.Process.GetProcessById((int)processId).ProcessName;
                    }
                    catch
                    {
                        return true;
                    }

                    if (!processName.Equals("Dofus", StringComparison.OrdinalIgnoreCase))
                    {
                        return true;
                    }

                    var style = GetWindowLongPtrSafe(hWnd, GWL_STYLE).ToInt64();
                    style &= ~WS_CHILD;
                    style |= WS_POPUP;
                    style |= WS_CAPTION;
                    style |= WS_THICKFRAME;
                    style |= WS_MINIMIZEBOX;
                    style |= WS_MAXIMIZEBOX;
                    style |= WS_SYSMENU;

                    SetParent(hWnd, IntPtr.Zero);
                    SetWindowLongPtrSafe(hWnd, GWL_STYLE, new IntPtr(style));
                    SetWindowPos(
                        hWnd,
                        IntPtr.Zero,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED);
                    ShowWindow(hWnd, SW_SHOW);

                    return true;
                }, IntPtr.Zero);
            }
        }

        private static IntPtr GetWindowLongPtrSafe(IntPtr hWnd, int index)
        {
            if (IntPtr.Size == 8)
            {
                return GetWindowLongPtr(hWnd, index);
            }

            return new IntPtr(GetWindowLong(hWnd, index));
        }

        private static void SetWindowLongPtrSafe(IntPtr hWnd, int index, IntPtr value)
        {
            if (IntPtr.Size == 8)
            {
                SetWindowLongPtr(hWnd, index, value);
            }
            else
            {
                SetWindowLong(hWnd, index, value.ToInt32());
            }
        }

        private sealed class EmbeddedWindowState
        {
            public IntPtr Handle { get; set; }
            public IntPtr OriginalParent { get; set; }
            public IntPtr OriginalStyle { get; set; }
            public IntPtr OriginalExStyle { get; set; }
            public int LastWidth { get; set; } = -1;
            public int LastHeight { get; set; } = -1;
        }

        [DllImport("user32.dll")]
        private static extern bool IsWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);

        private delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

        [DllImport("user32.dll")]
        private static extern bool IsIconic(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

        [DllImport("user32.dll", SetLastError = true)]
        private static extern IntPtr SetParent(IntPtr hWndChild, IntPtr hWndNewParent);

        [DllImport("user32.dll")]
        private static extern IntPtr GetParent(IntPtr hWnd);

        [DllImport("user32.dll")]
        private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

        [DllImport("user32.dll", EntryPoint = "GetWindowLong", SetLastError = true)]
        private static extern int GetWindowLong(IntPtr hWnd, int nIndex);

        [DllImport("user32.dll", EntryPoint = "GetWindowLongPtr", SetLastError = true)]
        private static extern IntPtr GetWindowLongPtr(IntPtr hWnd, int nIndex);

        [DllImport("user32.dll", EntryPoint = "SetWindowLong", SetLastError = true)]
        private static extern int SetWindowLong(IntPtr hWnd, int nIndex, int dwNewLong);

        [DllImport("user32.dll", EntryPoint = "SetWindowLongPtr", SetLastError = true)]
        private static extern IntPtr SetWindowLongPtr(IntPtr hWnd, int nIndex, IntPtr dwNewLong);

        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool SetWindowPos(
            IntPtr hWnd,
            IntPtr hWndInsertAfter,
            int x,
            int y,
            int cx,
            int cy,
            uint uFlags);
    }
}
