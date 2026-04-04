using System;
using System.Collections.Generic;

namespace DofusTabs.Application.Services
{
    public interface IEmbeddingRegistry
    {
        void Register(uint processId, IntPtr hwnd);
        void Unregister(uint processId);
        bool IsRegistered(uint processId);
        IReadOnlyList<(uint ProcessId, IntPtr Hwnd)> GetAll();
    }
}
