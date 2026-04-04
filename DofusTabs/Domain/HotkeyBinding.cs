using System.Collections.Generic;
using System.Windows.Input;

namespace DofusTabs.Domain
{
    public sealed record HotkeyBinding(ModifierKeys Modifiers, Key Key)
    {
        public static readonly HotkeyBinding None = new(ModifierKeys.None, Key.None);

        public bool IsEmpty => Key == Key.None;

        public override string ToString()
        {
            var parts = new List<string>();
            if ((Modifiers & ModifierKeys.Control) != 0) parts.Add("Ctrl");
            if ((Modifiers & ModifierKeys.Alt) != 0)     parts.Add("Alt");
            if ((Modifiers & ModifierKeys.Shift) != 0)   parts.Add("Shift");
            if (Key != Key.None) parts.Add(Key.ToString());
            return string.Join(" + ", parts);
        }
    }
}
