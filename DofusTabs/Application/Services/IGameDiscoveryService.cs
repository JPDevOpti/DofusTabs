using System;
using System.Collections.Generic;
using DofusTabs.Domain;

namespace DofusTabs.Application.Services
{
    public interface IGameDiscoveryService
    {
        IReadOnlyList<GameInstance> GetSnapshot(bool preserveState = true);

        bool TryGetWindowHandle(uint processId, out IntPtr handle);
    }
}
