using System;

namespace DofusTabs.Infrastructure.Embedding
{
    /// <summary>
    /// Estado Win32 original de una ventana antes de ser embebida.
    /// Usado por EmbeddingService para restaurarla fielmente.
    /// </summary>
    internal sealed class EmbeddedWindowSnapshot
    {
        public IntPtr Handle         { get; init; }
        public IntPtr OriginalParent { get; init; }
        public IntPtr OriginalStyle  { get; init; }
        public IntPtr OriginalExStyle { get; init; }
        public int LastWidth  { get; set; } = -1;
        public int LastHeight { get; set; } = -1;
    }
}
