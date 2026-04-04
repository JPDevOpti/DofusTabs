using System;

namespace DofusTabs.Infrastructure.Embedding
{
    internal sealed class EmbeddedWindowSnapshot
    {
        public IntPtr Handle          { get; init; }
        public IntPtr OriginalParent  { get; init; }
        public IntPtr OriginalStyle   { get; init; }
        public IntPtr OriginalExStyle { get; init; }
        public int LastWidth  { get; set; } = -1;
        public int LastHeight { get; set; } = -1;
    }
}
