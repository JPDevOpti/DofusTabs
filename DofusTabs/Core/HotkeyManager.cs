using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Input;
using System.Windows.Interop;

namespace DofusTabs.Core
{
    public class HotkeyManager : IDisposable
    {
        [DllImport("user32.dll")]
        private static extern bool RegisterHotKey(IntPtr hWnd, int id, uint fsModifiers, uint vk);

        [DllImport("user32.dll")]
        private static extern bool UnregisterHotKey(IntPtr hWnd, int id);

        private const int WM_HOTKEY = 0x0312;
        private const uint MOD_ALT = 0x0001;
        private const uint MOD_CONTROL = 0x0002;
        private const uint MOD_SHIFT = 0x0004;

        private IntPtr _windowHandle;
        private HwndSource? _source;
        private WindowManager _windowManager;

        private const int HOTKEY_ID_NEXT = 1;
        private const int HOTKEY_ID_PREVIOUS = 2;
        private const int HOTKEY_ID_INDIVIDUAL_START = 100;

        private HotkeyConfig _nextHotkey;
        private HotkeyConfig _previousHotkey;
        private Dictionary<uint, IndividualHotkeyInfo> _individualHotkeys = new Dictionary<uint, IndividualHotkeyInfo>();
        private int _nextIndividualHotkeyId = HOTKEY_ID_INDIVIDUAL_START;

        public event Action? OnNextWindow;
        public event Action? OnPreviousWindow;
        public event Action<WindowInfo>? OnIndividualHotkey;

        public HotkeyManager(Window window, WindowManager windowManager)
        {
            _windowManager = windowManager;
            _nextHotkey = new HotkeyConfig { Modifiers = ModifierKeys.Alt, Key = Key.Tab };
            _previousHotkey = new HotkeyConfig { Modifiers = ModifierKeys.Alt | ModifierKeys.Shift, Key = Key.Tab };
            Initialize(window);
        }

        private void Initialize(Window window)
        {
            window.Loaded += (s, e) =>
            {
                _windowHandle = new WindowInteropHelper(window).Handle;
                _source = HwndSource.FromHwnd(_windowHandle);
                _source?.AddHook(HwndHook);
                RegisterHotkeys();
            };

            window.Closed += (s, e) =>
            {
                UnregisterHotkeys();
                _source?.RemoveHook(HwndHook);
                _source?.Dispose();
            };
        }

        public void UpdateNextHotkey(ModifierKeys modifiers, Key key)
        {
            if (_windowHandle != IntPtr.Zero)
            {
                UnregisterHotKey(_windowHandle, HOTKEY_ID_NEXT);
            }
            _nextHotkey = new HotkeyConfig { Modifiers = modifiers, Key = key };
            if (_windowHandle != IntPtr.Zero)
            {
                RegisterHotKey(_windowHandle, HOTKEY_ID_NEXT, GetModifiersValue(modifiers), (uint)KeyInterop.VirtualKeyFromKey(key));
            }
        }

        public void UpdatePreviousHotkey(ModifierKeys modifiers, Key key)
        {
            if (_windowHandle != IntPtr.Zero)
            {
                UnregisterHotKey(_windowHandle, HOTKEY_ID_PREVIOUS);
            }
            _previousHotkey = new HotkeyConfig { Modifiers = modifiers, Key = key };
            if (_windowHandle != IntPtr.Zero)
            {
                RegisterHotKey(_windowHandle, HOTKEY_ID_PREVIOUS, GetModifiersValue(modifiers), (uint)KeyInterop.VirtualKeyFromKey(key));
            }
        }

        private uint GetModifiersValue(ModifierKeys modifiers)
        {
            uint value = 0;
            if ((modifiers & ModifierKeys.Alt) != 0) value |= MOD_ALT;
            if ((modifiers & ModifierKeys.Control) != 0) value |= MOD_CONTROL;
            if ((modifiers & ModifierKeys.Shift) != 0) value |= MOD_SHIFT;
            return value;
        }

        private void RegisterHotkeys()
        {
            if (_windowHandle != IntPtr.Zero)
            {
                RegisterHotKey(_windowHandle, HOTKEY_ID_NEXT, GetModifiersValue(_nextHotkey.Modifiers), (uint)KeyInterop.VirtualKeyFromKey(_nextHotkey.Key));
                RegisterHotKey(_windowHandle, HOTKEY_ID_PREVIOUS, GetModifiersValue(_previousHotkey.Modifiers), (uint)KeyInterop.VirtualKeyFromKey(_previousHotkey.Key));
            }
        }

        public void ReRegisterHotkeys()
        {
            // Re-registrar todos los atajos (útil después de cargar configuración)
            if (_windowHandle != IntPtr.Zero)
            {
                UnregisterHotKey(_windowHandle, HOTKEY_ID_NEXT);
                UnregisterHotKey(_windowHandle, HOTKEY_ID_PREVIOUS);
                RegisterHotkeys();
                
                // Re-registrar atajos individuales
                foreach (var hotkey in _individualHotkeys.Values.ToList())
                {
                    UnregisterHotKey(_windowHandle, hotkey.HotkeyId);
                    RegisterHotKey(_windowHandle, hotkey.HotkeyId, GetModifiersValue(hotkey.Modifiers), (uint)KeyInterop.VirtualKeyFromKey(hotkey.Key));
                }
            }
        }

        private void UnregisterHotkeys()
        {
            UnregisterHotKey(_windowHandle, HOTKEY_ID_NEXT);
            UnregisterHotKey(_windowHandle, HOTKEY_ID_PREVIOUS);
        }

        public string GetNextHotkeyDisplay()
        {
            return FormatHotkey(_nextHotkey);
        }

        public string GetPreviousHotkeyDisplay()
        {
            return FormatHotkey(_previousHotkey);
        }

        public HotkeyConfig GetNextHotkeyConfig()
        {
            return _nextHotkey;
        }

        public HotkeyConfig GetPreviousHotkeyConfig()
        {
            return _previousHotkey;
        }

        private string FormatHotkey(HotkeyConfig config)
        {
            string result = "";
            if ((config.Modifiers & ModifierKeys.Control) != 0) result += "Ctrl + ";
            if ((config.Modifiers & ModifierKeys.Alt) != 0) result += "Alt + ";
            if ((config.Modifiers & ModifierKeys.Shift) != 0) result += "Shift + ";
            result += config.Key.ToString();
            return result;
        }

        public class HotkeyConfig
        {
            public ModifierKeys Modifiers { get; set; }
            public Key Key { get; set; }
        }

        private IntPtr HwndHook(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
        {
            if (msg == WM_HOTKEY)
            {
                int id = wParam.ToInt32();
                switch (id)
                {
                    case HOTKEY_ID_NEXT:
                        OnNextWindow?.Invoke();
                        handled = true;
                        break;
                    case HOTKEY_ID_PREVIOUS:
                        OnPreviousWindow?.Invoke();
                        handled = true;
                        break;
                    default:
                        // Buscar en atajos individuales
                        if (id >= HOTKEY_ID_INDIVIDUAL_START)
                        {
                            var hotkey = _individualHotkeys.Values.FirstOrDefault(h => h.HotkeyId == id);
                            if (hotkey != null)
                            {
                                OnIndividualHotkey?.Invoke(hotkey.WindowInfo);
                                handled = true;
                            }
                        }
                        break;
                }
            }
            return IntPtr.Zero;
        }

        public void RegisterIndividualHotkey(WindowInfo windowInfo, ModifierKeys modifiers, Key key)
        {
            if (_windowHandle == IntPtr.Zero)
            {
                return; // No se puede registrar si el handle no está disponible
            }

            // Desregistrar el atajo anterior si existe
            if (_individualHotkeys.ContainsKey(windowInfo.ProcessId))
            {
                var oldHotkey = _individualHotkeys[windowInfo.ProcessId];
                UnregisterHotKey(_windowHandle, oldHotkey.HotkeyId);
                _individualHotkeys.Remove(windowInfo.ProcessId);
            }

            // Registrar el nuevo atajo
            int hotkeyId = _nextIndividualHotkeyId++;
            if (RegisterHotKey(_windowHandle, hotkeyId, GetModifiersValue(modifiers), (uint)KeyInterop.VirtualKeyFromKey(key)))
            {
                _individualHotkeys[windowInfo.ProcessId] = new IndividualHotkeyInfo
                {
                    HotkeyId = hotkeyId,
                    WindowInfo = windowInfo,
                    Modifiers = modifiers,
                    Key = key
                };
            }
        }

        public void UnregisterIndividualHotkey(uint processId)
        {
            if (_individualHotkeys.ContainsKey(processId))
            {
                var hotkey = _individualHotkeys[processId];
                UnregisterHotKey(_windowHandle, hotkey.HotkeyId);
                _individualHotkeys.Remove(processId);
            }
        }

        private void UnregisterAllIndividualHotkeys()
        {
            foreach (var hotkey in _individualHotkeys.Values)
            {
                UnregisterHotKey(_windowHandle, hotkey.HotkeyId);
            }
            _individualHotkeys.Clear();
        }

        public void Dispose()
        {
            UnregisterHotkeys();
            UnregisterAllIndividualHotkeys();
            _source?.Dispose();
        }

        private class IndividualHotkeyInfo
        {
            public int HotkeyId { get; set; }
            public WindowInfo WindowInfo { get; set; } = null!;
            public ModifierKeys Modifiers { get; set; }
            public Key Key { get; set; }
        }
    }
}

