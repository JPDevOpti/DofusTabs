using System;

namespace DofusTabs.Application.Services
{
    public interface IEmbeddingService
    {
        /// <summary>Embebe gameWindowHandle dentro de hostHandle. Restaura cualquier ventana previamente embebida.</summary>
        bool TryEmbed(uint processId, IntPtr gameWindowHandle, IntPtr hostHandle, int width, int height);

        /// <summary>Redimensiona la ventana embebida actualmente activa.</summary>
        void Resize(int width, int height);

        /// <summary>Restaura la ventana del processId dado a su estado original.</summary>
        void Restore(uint processId);

        /// <summary>Restaura todas las ventanas embebidas registradas.</summary>
        void RestoreAll();

        /// <summary>Indica si el processId tiene una ventana actualmente embebida.</summary>
        bool IsEmbedded(uint processId);
    }
}
