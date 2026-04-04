using System;
using DofusTabs.Application.Services;
using DofusTabs.Diagnostics;
using DofusTabs.Infrastructure.Win32;

namespace DofusTabs.Infrastructure.Embedding
{
    /// <summary>
    /// Recupera ventanas embebidas a su estado original.
    ///
    /// Estrategia dual:
    ///   1. Prioridad: usa IEmbeddingRegistry (sabe exactamente qué está embebido).
    ///   2. Fallback estático: EnumWindows sobre KnownProcessNames — para crashes donde
    ///      el registro no está disponible (startup/unhandled exception antes de DI).
    /// </summary>
    public sealed class RecoveryService : IRecoveryService
    {
        private readonly IEmbeddingService _embeddingService;

        public RecoveryService(IEmbeddingService embeddingService)
        {
            _embeddingService = embeddingService;
        }

        public void RecoverAll(string reason)
        {
            AppLogger.Info($"RecoveryService.RecoverAll: {reason}");
            try
            {
                _embeddingService.RestoreAll();
            }
            catch (Exception ex)
            {
                AppLogger.Error(ex, "RecoveryService: fallo al usar EmbeddingService, aplicando recovery estático");
                EnsureCleanState();
            }
        }

        /// <summary>
        /// Recovery estático — no requiere DI. Llamar desde App antes de que el container esté listo,
        /// o como último recurso desde un unhandled exception handler.
        ///
        /// Cubre: "dofus" y "dofustouch" (todos los nombres conocidos del juego).
        /// </summary>
        public static void EnsureCleanState()
        {
            try
            {
                User32.EnumWindows((hWnd, _) =>
                {
                    if (!User32.IsWindow(hWnd)) return true;

                    var parent = User32.GetParent(hWnd);
                    if (parent == IntPtr.Zero) return true;

                    User32.GetWindowThreadProcessId(hWnd, out uint pid);
                    if (pid == 0) return true;

                    string processName;
                    try
                    {
                        processName = System.Diagnostics.Process.GetProcessById((int)pid).ProcessName;
                    }
                    catch { return true; }

                    if (!EmbeddingRegistry.KnownProcessNames.Contains(processName))
                        return true;

                    // Esta ventana del juego tiene un padre — restaurar
                    long style = User32.GetWindowLong(hWnd, User32.GWL_STYLE).ToInt64();
                    style &= ~User32.WS_CHILD;
                    style |= User32.WS_POPUP;
                    style |= User32.WS_CAPTION;
                    style |= User32.WS_THICKFRAME;
                    style |= User32.WS_MINIMIZEBOX;
                    style |= User32.WS_MAXIMIZEBOX;
                    style |= User32.WS_SYSMENU;

                    User32.SetParent(hWnd, IntPtr.Zero);
                    User32.SetWindowLong(hWnd, User32.GWL_STYLE, new IntPtr(style));
                    User32.SetWindowPos(hWnd, IntPtr.Zero, 0, 0, 0, 0,
                        User32.SWP_NOMOVE | User32.SWP_NOSIZE | User32.SWP_NOZORDER |
                        User32.SWP_NOOWNERZORDER | User32.SWP_FRAMECHANGED);
                    User32.ShowWindow(hWnd, User32.SW_SHOW);

                    return true;
                }, IntPtr.Zero);
            }
            catch
            {
                // El recovery nunca debe lanzar excepciones
            }
        }
    }
}
