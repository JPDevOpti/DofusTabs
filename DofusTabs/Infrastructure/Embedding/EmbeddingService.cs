using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using DofusTabs.Application.Services;
using DofusTabs.Diagnostics;
using DofusTabs.Infrastructure.Win32;

namespace DofusTabs.Infrastructure.Embedding
{
    public sealed class EmbeddingService : IEmbeddingService, IDisposable
    {
        private readonly EmbeddingRegistry _registry;
        private uint? _visibleProcessId;

        public EmbeddingService(IEmbeddingRegistry registry)
        {
            _registry = (EmbeddingRegistry)registry;
        }

        public IReadOnlyList<uint> EmbeddedProcessIds =>
            _registry.GetAll().Select(x => x.ProcessId).ToList();

        public bool TryEmbed(uint processId, IntPtr gameHandle, IntPtr hostHandle, int width, int height)
        {
            if (gameHandle == IntPtr.Zero || hostHandle == IntPtr.Zero)
                return false;

            if (!User32.IsWindow(gameHandle))
            {
                AppLogger.Warn($"TryEmbed: handle inválido para proceso {processId}");
                return false;
            }

            if (_registry.IsRegistered(processId))
            {
                SwitchVisible(processId, width, height);
                return true;
            }

            if (User32.IsIconic(gameHandle))
                User32.ShowWindow(gameHandle, User32.SW_RESTORE);

            var snapshot = new EmbeddedWindowSnapshot
            {
                Handle          = gameHandle,
                OriginalParent  = User32.GetParent(gameHandle),
                OriginalStyle   = User32.GetWindowLong(gameHandle, User32.GWL_STYLE),
                OriginalExStyle = User32.GetWindowLong(gameHandle, User32.GWL_EXSTYLE),
            };

            if (User32.SetParent(gameHandle, hostHandle) == IntPtr.Zero)
            {
                int err = Marshal.GetLastWin32Error();
                if (err != 0)
                {
                    AppLogger.Warn($"TryEmbed: SetParent falló para proceso {processId} (error {err})");
                    return false;
                }
            }

            long style = snapshot.OriginalStyle.ToInt64();
            style |=  User32.WS_CHILD;
            style &= ~User32.WS_POPUP;
            style &= ~User32.WS_CAPTION;
            style &= ~User32.WS_THICKFRAME;
            style &= ~User32.WS_MINIMIZEBOX;
            style &= ~User32.WS_MAXIMIZEBOX;
            style &= ~User32.WS_SYSMENU;

            long exStyle = snapshot.OriginalExStyle.ToInt64();
            exStyle &= ~User32.WS_EX_DLGMODALFRAME;
            exStyle &= ~User32.WS_EX_CLIENTEDGE;
            exStyle &= ~User32.WS_EX_STATICEDGE;

            User32.SetWindowLong(gameHandle, User32.GWL_STYLE,   new IntPtr(style));
            User32.SetWindowLong(gameHandle, User32.GWL_EXSTYLE, new IntPtr(exStyle));

            _registry.Register(processId, gameHandle);
            _registry.RegisterSnapshot(processId, snapshot);

            User32.SetWindowPos(gameHandle, IntPtr.Zero, 0, 0, 0, 0,
                User32.SWP_NOMOVE | User32.SWP_NOSIZE | User32.SWP_NOZORDER |
                User32.SWP_NOOWNERZORDER | User32.SWP_FRAMECHANGED);

            SwitchVisible(processId, width, height);

            AppLogger.Info($"EmbeddingService: embebido proceso {processId}");
            return true;
        }

        public void Resize(int width, int height)
        {
            if (!_visibleProcessId.HasValue) return;
            if (_registry.TryGetSnapshot(_visibleProcessId.Value, out var snapshot) && snapshot != null)
                ResizeTo(snapshot, width, height);
        }

        public void Restore(uint processId) => RestoreInternal(processId);

        public void RestoreAll()
        {
            foreach (var (pid, _) in _registry.GetAll())
                RestoreInternal(pid);
            _visibleProcessId = null;
        }

        public bool IsEmbedded(uint processId) => _registry.IsRegistered(processId);

        public void Dispose() => RestoreAll();

        private void HideVisible()
        {
            if (!_visibleProcessId.HasValue) return;
            if (_registry.TryGetSnapshot(_visibleProcessId.Value, out var snapshot) && snapshot != null)
                User32.ShowWindow(snapshot.Handle, User32.SW_HIDE);
            _visibleProcessId = null;
        }

        private void ShowEmbedded(uint processId, int width, int height)
        {
            if (!_registry.TryGetSnapshot(processId, out var snapshot) || snapshot == null) return;
            ResizeTo(snapshot, width, height);
            User32.ShowWindow(snapshot.Handle, User32.SW_SHOW);
            _visibleProcessId = processId;
        }

        private void SwitchVisible(uint processId, int width, int height)
        {
            if (!_registry.TryGetSnapshot(processId, out var target) || target == null)
                return;

            int w = Math.Max(8, width);
            int h = Math.Max(8, height);

            EmbeddedWindowSnapshot? previous = null;
            if (_visibleProcessId.HasValue && _visibleProcessId.Value != processId)
                _registry.TryGetSnapshot(_visibleProcessId.Value, out previous);

            bool switchedAtomically = false;

            if (previous != null && User32.IsWindow(previous.Handle))
            {
                var defer = User32.BeginDeferWindowPos(2);
                if (defer != IntPtr.Zero)
                {
                    defer = User32.DeferWindowPos(
                        defer,
                        target.Handle,
                        IntPtr.Zero,
                        0, 0, w, h,
                        User32.SWP_NOOWNERZORDER | User32.SWP_SHOWWINDOW);

                    if (defer != IntPtr.Zero)
                    {
                        defer = User32.DeferWindowPos(
                            defer,
                            previous.Handle,
                            IntPtr.Zero,
                            0, 0, 0, 0,
                            User32.SWP_NOMOVE | User32.SWP_NOSIZE | User32.SWP_NOZORDER |
                            User32.SWP_NOOWNERZORDER | User32.SWP_NOACTIVATE | User32.SWP_HIDEWINDOW);
                    }

                    if (defer != IntPtr.Zero)
                        switchedAtomically = User32.EndDeferWindowPos(defer);
                }
            }

            if (!switchedAtomically)
            {
                User32.SetWindowPos(
                    target.Handle,
                    IntPtr.Zero,
                    0, 0, w, h,
                    User32.SWP_NOOWNERZORDER | User32.SWP_SHOWWINDOW);

                if (previous != null && User32.IsWindow(previous.Handle))
                    User32.ShowWindow(previous.Handle, User32.SW_HIDE);
            }

            target.LastWidth = w;
            target.LastHeight = h;
            _visibleProcessId = processId;
        }

        private void ResizeTo(EmbeddedWindowSnapshot snapshot, int width, int height)
        {
            int w = Math.Max(8, width);
            int h = Math.Max(8, height);
            if (snapshot.LastWidth == w && snapshot.LastHeight == h) return;
            User32.SetWindowPos(snapshot.Handle, IntPtr.Zero, 0, 0, w, h,
                User32.SWP_NOZORDER | User32.SWP_NOOWNERZORDER | User32.SWP_NOACTIVATE);
            snapshot.LastWidth  = w;
            snapshot.LastHeight = h;
        }

        private void RestoreInternal(uint processId)
        {
            if (!_registry.TryGetSnapshot(processId, out var snapshot) || snapshot == null)
            {
                _registry.Unregister(processId);
                if (_visibleProcessId == processId) _visibleProcessId = null;
                return;
            }

            if (!User32.IsWindow(snapshot.Handle))
            {
                _registry.Unregister(processId);
                if (_visibleProcessId == processId) _visibleProcessId = null;
                return;
            }

            User32.SetParent(snapshot.Handle, snapshot.OriginalParent);
            User32.SetWindowLong(snapshot.Handle, User32.GWL_STYLE,   snapshot.OriginalStyle);
            User32.SetWindowLong(snapshot.Handle, User32.GWL_EXSTYLE, snapshot.OriginalExStyle);

            User32.SetWindowPos(snapshot.Handle, IntPtr.Zero, 0, 0, 0, 0,
                User32.SWP_NOMOVE | User32.SWP_NOSIZE | User32.SWP_NOZORDER |
                User32.SWP_NOOWNERZORDER | User32.SWP_FRAMECHANGED);

            User32.ShowWindow(snapshot.Handle, User32.SW_SHOW);
            _registry.Unregister(processId);

            if (_visibleProcessId == processId)
                _visibleProcessId = null;

            AppLogger.Info($"EmbeddingService: restaurado proceso {processId}");
        }
    }
}
