using System;
using System.Collections.Generic;
using DofusTabs.Domain;

namespace DofusTabs.Application.Services
{
    public interface IGameDiscoveryService
    {
        /// <summary>
        /// Retorna snapshot de instancias activas. Con preserveState=true,
        /// mantiene DisplayOrder/IsEnabled/IndividualHotkey de la llamada anterior.
        /// </summary>
        IReadOnlyList<GameInstance> GetSnapshot(bool preserveState = true);

        /// <summary>Obtiene el HWND de la ventana asociada al processId detectado.</summary>
        bool TryGetWindowHandle(uint processId, out IntPtr handle);
    }
}
