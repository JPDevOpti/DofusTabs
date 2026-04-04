using System;
using System.Windows;
using DofusTabs.Domain;

namespace DofusTabs.Application.Services
{
    public interface IHotkeyService : IDisposable
    {
        void Initialize(Window window);

        bool Register(int id, HotkeyBinding binding);
        void Unregister(int id);

        bool RegisterForInstance(uint processId, HotkeyBinding binding);
        void UnregisterForInstance(uint processId);
        void UnregisterAll();

        void SetSuspended(bool suspended);

        int NextHotkeyId { get; }
        int PreviousHotkeyId { get; }
        int PrimaryHotkeyId { get; }

        event Action? NextRequested;
        event Action? PreviousRequested;
        event Action? PrimaryRequested;
        event Action<uint>? InstanceActivationRequested;
    }
}
