using System;
using System.Runtime.InteropServices;
using DofusTabs.Application.Services;
using DofusTabs.Diagnostics;
using DofusTabs.Infrastructure.Win32;

namespace DofusTabs.Infrastructure.Embedding
{
    public sealed class EmbeddingService : IEmbeddingService, IDisposable
    {
        private readonly EmbeddingRegistry _registry;
        private uint? _activeProcessId;

        public EmbeddingService(IEmbeddingRegistry registry)
        {
            _registry = (EmbeddingRegistry)registry;
        }

        public bool TryEmbed(uint processId, IntPtr gameHandle, IntPtr hostHandle, int width, int height)
        {
            if (gameHandle == IntPtr.Zero || hostHandle == IntPtr.Zero)
                return false;

            if (!User32.IsWindow(gameHandle))
            {
                AppLogger.Warn($"TryEmbed: handle inválido para proceso {processId}");
                return false;
            }

            // Si ya está embebida esta misma ventana, solo redimensionar
            if (_activeProcessId == processId && _registry.IsRegistered(processId))
            {
                ResizeInternal(processId, width, height);
                return true;
            }

            // Restaurar la ventana embebida anterior
            if (_activeProcessId.HasValue)
                RestoreInternal(_activeProcessId.Value);

            // Restaurar desde minimizado si aplica
            if (User32.IsIconic(gameHandle))
                User32.ShowWindow(gameHandle, User32.SW_RESTORE);

            // Guardar snapshot del estado original
            var snapshot = new EmbeddedWindowSnapshot
            {
                Handle         = gameHandle,
                OriginalParent = User32.GetParent(gameHandle),
                OriginalStyle  = User32.GetWindowLong(gameHandle, User32.GWL_STYLE),
                OriginalExStyle = User32.GetWindowLong(gameHandle, User32.GWL_EXSTYLE),
            };

            // SetParent
            if (User32.SetParent(gameHandle, hostHandle) == IntPtr.Zero && Marshal.GetLastWin32Error() != 0)
            {
                AppLogger.Warn($"TryEmbed: SetParent falló para proceso {processId} (error {Marshal.GetLastWin32Error()})");
                return false;
            }

            // Aplicar estilo de ventana hijo (sin bordes, sin barra de título)
            long style = snapshot.OriginalStyle.ToInt64();
            style |= User32.WS_CHILD;
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

            User32.SetWindowLong(gameHandle, User32.GWL_STYLE, new IntPtr(style));
            User32.SetWindowLong(gameHandle, User32.GWL_EXSTYLE, new IntPtr(exStyle));

            // Registrar en registry con snapshot completo
            _registry.Register(processId, gameHandle);
            _registry.RegisterSnapshot(processId, snapshot);
            _activeProcessId = processId;

            // Forzar recálculo de frame y luego posicionar
            User32.SetWindowPos(gameHandle, IntPtr.Zero, 0, 0, 0, 0,
                User32.SWP_NOMOVE | User32.SWP_NOSIZE | User32.SWP_NOZORDER |
                User32.SWP_NOOWNERZORDER | User32.SWP_FRAMECHANGED);

            ResizeInternal(processId, width, height);
            User32.ShowWindow(gameHandle, User32.SW_SHOW);

            AppLogger.Info($"EmbeddingService: embebido proceso {processId}");
            return true;
        }

        public void Resize(int width, int height)
        {
            if (_activeProcessId.HasValue)
                ResizeInternal(_activeProcessId.Value, width, height);
        }

        public void Restore(uint processId) => RestoreInternal(processId);

        public void RestoreAll()
        {
            foreach (var (pid, _) in _registry.GetAll())
                RestoreInternal(pid);
            _activeProcessId = null;
        }

        public bool IsEmbedded(uint processId) => _registry.IsRegistered(processId);

        public void Dispose() => RestoreAll();

        // ── Privados ─────────────────────────────────────────────────────────

        private void ResizeInternal(uint processId, int width, int height)
        {
            if (!_registry.TryGetSnapshot(processId, out var snapshot) || snapshot == null)
                return;

            int w = Math.Max(8, width);
            int h = Math.Max(8, height);

            if (snapshot.LastWidth == w && snapshot.LastHeight == h) return;

            User32.SetWindowPos(snapshot.Handle, IntPtr.Zero, 0, 0, w, h,
                User32.SWP_NOZORDER | User32.SWP_NOOWNERZORDER | User32.SWP_NOACTIVATE);

            snapshot.LastWidth = w;
            snapshot.LastHeight = h;
        }

        private void RestoreInternal(uint processId)
        {
            if (!_registry.TryGetSnapshot(processId, out var snapshot) || snapshot == null)
            {
                _registry.Unregister(processId);
                return;
            }

            if (!User32.IsWindow(snapshot.Handle))
            {
                _registry.Unregister(processId);
                return;
            }

            User32.SetParent(snapshot.Handle, snapshot.OriginalParent);
            User32.SetWindowLong(snapshot.Handle, User32.GWL_STYLE, snapshot.OriginalStyle);
            User32.SetWindowLong(snapshot.Handle, User32.GWL_EXSTYLE, snapshot.OriginalExStyle);

            User32.SetWindowPos(snapshot.Handle, IntPtr.Zero, 0, 0, 0, 0,
                User32.SWP_NOMOVE | User32.SWP_NOSIZE | User32.SWP_NOZORDER |
                User32.SWP_NOOWNERZORDER | User32.SWP_FRAMECHANGED);

            User32.ShowWindow(snapshot.Handle, User32.SW_SHOW);
            _registry.Unregister(processId);

            if (_activeProcessId == processId)
                _activeProcessId = null;

            AppLogger.Info($"EmbeddingService: restaurado proceso {processId}");
        }
    }
}
