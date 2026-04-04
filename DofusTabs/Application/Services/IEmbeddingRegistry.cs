using System;
using System.Collections.Generic;

namespace DofusTabs.Application.Services
{
    /// <summary>
    /// Registro centralizado de ventanas actualmente embebidas.
    /// Permite que RecoveryService sepa exactamente qué restaurar sin recurrir a EnumWindows.
    /// </summary>
    public interface IEmbeddingRegistry
    {
        void Register(uint processId, IntPtr hwnd);
        void Unregister(uint processId);
        bool IsRegistered(uint processId);
        IReadOnlyList<(uint ProcessId, IntPtr Hwnd)> GetAll();
    }
}
