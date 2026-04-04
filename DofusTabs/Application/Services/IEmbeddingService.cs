using System;
using System.Collections.Generic;

namespace DofusTabs.Application.Services
{
    public interface IEmbeddingService
    {
        bool TryEmbed(uint processId, IntPtr gameWindowHandle, IntPtr hostHandle, int width, int height);

        void Resize(int width, int height);

        void Restore(uint processId);

        void RestoreAll();

        bool IsEmbedded(uint processId);

        IReadOnlyList<uint> EmbeddedProcessIds { get; }
    }
}
