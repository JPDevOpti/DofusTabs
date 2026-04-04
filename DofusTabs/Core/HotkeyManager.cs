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

        private const int HOTKEY_ID_NEXT = 1;
        private const int HOTKEY_ID_PREVIOUS = 2;
        private const int HOTKEY_ID_INDIVIDUAL_START = 100;

        private HotkeyConfig _nextHotkey;
        private HotkeyConfig _previousHotkey;
        private bool _nextHotkeyEnabled;
        private bool _previousHotkeyEnabled;
        private Dictionary<uint, IndividualHotkeyInfo> _individualHotkeys = new Dictionary<uint, IndividualHotkeyInfo>();
        private int _nextIndividualHotkeyId = HOTKEY_ID_INDIVIDUAL_START;
        private bool _suspendHotkeyActions = false;

        public event Action? OnNextWindow;
        public event Action? OnPreviousWindow;
        public event Action<WindowInfo>? OnIndividualHotkey;

        public void SetHotkeyActionsSuspended(bool suspended)
        {
            _suspendHotkeyActions = suspended;
        }

        public HotkeyManager(Window window)
        {
            _nextHotkey = new HotkeyConfig { Modifiers = ModifierKeys.Alt, Key = Key.Tab };
            _previousHotkey = new HotkeyConfig { Modifiers = ModifierKeys.Alt | ModifierKeys.Shift, Key = Key.Tab };
            _nextHotkeyEnabled = true;
            _previousHotkeyEnabled = true;
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

        public bool UpdateNextHotkey(ModifierKeys modifiers, Key key)
        {
            return UpdateHotkeyCore(HOTKEY_ID_NEXT, modifiers, key, isNext: true);
        }

        public bool UpdatePreviousHotkey(ModifierKeys modifiers, Key key)
        {
            return UpdateHotkeyCore(HOTKEY_ID_PREVIOUS, modifiers, key, isNext: false);
        }

        public void ClearNextHotkey()
        {
            _ = UpdateNextHotkey(ModifierKeys.None, Key.None);
        }

        public void ClearPreviousHotkey()
        {
            _ = UpdatePreviousHotkey(ModifierKeys.None, Key.None);
        }

        private bool UpdateHotkeyCore(int hotkeyId, ModifierKeys modifiers, Key key, bool isNext)
        {
            var oldConfig = isNext ? _nextHotkey : _previousHotkey;
            bool oldEnabled = isNext ? _nextHotkeyEnabled : _previousHotkeyEnabled;

            var newConfig = new HotkeyConfig { Modifiers = modifiers, Key = key };
            bool newEnabled = key != Key.None;

            if (isNext)
            {
                _nextHotkey = newConfig;
                _nextHotkeyEnabled = newEnabled;
            }
            else
            {
                _previousHotkey = newConfig;
                _previousHotkeyEnabled = newEnabled;
            }

            if (_windowHandle == IntPtr.Zero)
            {
                return true;
            }

            UnregisterHotKey(_windowHandle, hotkeyId);

            if (!newEnabled)
            {
                return true;
            }

            uint modifiersValue = GetModifiersValue(modifiers);
            uint virtualKey = (uint)KeyInterop.VirtualKeyFromKey(key);
            if (RegisterHotKey(_windowHandle, hotkeyId, modifiersValue, virtualKey))
            {
                return true;
            }

            if (isNext)
            {
                _nextHotkey = oldConfig;
                _nextHotkeyEnabled = oldEnabled;
            }
            else
            {
                _previousHotkey = oldConfig;
                _previousHotkeyEnabled = oldEnabled;
            }

            if (oldEnabled)
            {
                RegisterHotKey(
                    _windowHandle,
                    hotkeyId,
                    GetModifiersValue(oldConfig.Modifiers),
                    (uint)KeyInterop.VirtualKeyFromKey(oldConfig.Key));
            }

            return false;
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
                if (_nextHotkeyEnabled)
                {
                    RegisterHotKey(_windowHandle, HOTKEY_ID_NEXT, GetModifiersValue(_nextHotkey.Modifiers), (uint)KeyInterop.VirtualKeyFromKey(_nextHotkey.Key));
                }

                if (_previousHotkeyEnabled)
                {
                    RegisterHotKey(_windowHandle, HOTKEY_ID_PREVIOUS, GetModifiersValue(_previousHotkey.Modifiers), (uint)KeyInterop.VirtualKeyFromKey(_previousHotkey.Key));
                }
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
            return _nextHotkeyEnabled ? FormatHotkey(_nextHotkey) : "Ninguno";
        }

        public string GetPreviousHotkeyDisplay()
        {
            return _previousHotkeyEnabled ? FormatHotkey(_previousHotkey) : "Ninguno";
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
                // Durante la captura de atajos, evitar que se ejecute cualquier acción
                if (_suspendHotkeyActions)
                {
                    handled = true;
                    return IntPtr.Zero;
                }

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

        public bool RegisterIndividualHotkey(WindowInfo windowInfo, ModifierKeys modifiers, Key key)
        {
            if (_windowHandle == IntPtr.Zero)
            {
                return false; // No se puede registrar si el handle no está disponible
            }

            // Buscar y desregistrar cualquier otro atajo que use la misma combinación
            var conflictHotkey = _individualHotkeys.Values.FirstOrDefault(h => 
                h.Modifiers == modifiers && h.Key == key && h.WindowInfo.ProcessId != windowInfo.ProcessId);
            
            if (conflictHotkey != null)
            {
                UnregisterHotKey(_windowHandle, conflictHotkey.HotkeyId);
                _individualHotkeys.Remove(conflictHotkey.WindowInfo.ProcessId);
            }

            // Desregistrar el atajo anterior de esta ventana si existe
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

                return true;
            }

            return false;
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

