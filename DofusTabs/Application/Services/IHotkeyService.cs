using System;
using System.Windows;
using DofusTabs.Domain;

namespace DofusTabs.Application.Services
{
    public interface IHotkeyService : IDisposable
    {
        /// <summary>
        /// Debe llamarse desde Window.Loaded para registrar los hooks Win32.
        /// No puede hacerse en el constructor porque el HWND aún no existe.
        /// </summary>
        void Initialize(Window window);

        bool Register(int id, HotkeyBinding binding);
        void Unregister(int id);

        bool RegisterForInstance(uint processId, HotkeyBinding binding);
        void UnregisterForInstance(uint processId);
        void UnregisterAll();

        /// <summary>Suspende la ejecución de acciones de hotkey (útil durante captura de nueva combinación).</summary>
        void SetSuspended(bool suspended);

        // IDs predefinidos para hotkeys globales
        int NextHotkeyId { get; }
        int PreviousHotkeyId { get; }

        event Action? NextRequested;
        event Action? PreviousRequested;
        event Action<uint>? InstanceActivationRequested;
    }
}
