using System;
using System.Collections.Generic;
using System.Linq;
using System.Windows;
using System.Windows.Input;
using System.Windows.Interop;
using DofusTabs.Application.Services;
using DofusTabs.Diagnostics;
using DofusTabs.Domain;
using DofusTabs.Infrastructure.Win32;

namespace DofusTabs.Infrastructure.Hotkeys
{
    public sealed class HotkeyService : IHotkeyService
    {
        public int NextHotkeyId     => 1;
        public int PreviousHotkeyId => 2;
        public int PrimaryHotkeyId  => 3;
        private const int InstanceHotkeyBase = 100;

        public event Action? NextRequested;
        public event Action? PreviousRequested;
        public event Action? PrimaryRequested;
        public event Action<uint>? InstanceActivationRequested;

        private IntPtr _hwnd;
        private HwndSource? _hwndSource;
        private bool _suspended;

        private readonly Dictionary<int, HotkeyBinding> _globalHotkeys = new();
        private readonly Dictionary<uint, (int HotkeyId, HotkeyBinding Binding)> _instanceHotkeys = new();
        private int _nextInstanceId = InstanceHotkeyBase;

        public void Initialize(Window window)
        {
            void AttachHooks()
            {
                if (_hwnd != IntPtr.Zero) return;

                _hwnd = new WindowInteropHelper(window).Handle;
                _hwndSource = HwndSource.FromHwnd(_hwnd);
                _hwndSource?.AddHook(HwndHook);
                ReRegisterAll();
                AppLogger.Info("HotkeyService inicializado");
            }

            if (window.IsLoaded)
                AttachHooks();
            else
                window.Loaded += (_, _) => AttachHooks();

            window.Closed += (_, _) =>
            {
                UnregisterAll();
                _hwndSource?.RemoveHook(HwndHook);
                _hwndSource?.Dispose();
            };
        }

        public bool Register(int id, HotkeyBinding binding)
        {
            if (!IsValidBinding(binding))
            {
                AppLogger.Warn($"HotkeyService: hotkey global inválido id={id} ({binding}).");
                Unregister(id);
                return false;
            }

            if (_hwnd != IntPtr.Zero)
                User32.UnregisterHotKey(_hwnd, id);

            _globalHotkeys[id] = binding;

            if (_hwnd == IntPtr.Zero) return true;

            uint mods = ModifiersToWin32(binding.Modifiers);
            uint vk   = (uint)KeyInterop.VirtualKeyFromKey(binding.Key);
            bool ok   = User32.RegisterHotKey(_hwnd, id, mods, vk);
            if (!ok)
                AppLogger.Warn($"HotkeyService: no se pudo registrar hotkey id={id} ({binding})");
            return ok;
        }

        public void Unregister(int id)
        {
            _globalHotkeys.Remove(id);
            if (_hwnd != IntPtr.Zero)
                User32.UnregisterHotKey(_hwnd, id);
        }

        public bool RegisterForInstance(uint processId, HotkeyBinding binding)
        {
            UnregisterForInstance(processId);

            if (!IsValidBinding(binding))
            {
                AppLogger.Warn($"HotkeyService: hotkey de instancia inválido pid={processId} ({binding}).");
                return false;
            }

            if (_hwnd == IntPtr.Zero) return false;

            var conflict = _instanceHotkeys
                .FirstOrDefault(kvp => kvp.Key != processId && kvp.Value.Binding == binding);
            if (conflict.Key != 0)
                UnregisterForInstance(conflict.Key);

            int hotkeyId = _nextInstanceId++;
            uint mods = ModifiersToWin32(binding.Modifiers);
            uint vk   = (uint)KeyInterop.VirtualKeyFromKey(binding.Key);

            if (User32.RegisterHotKey(_hwnd, hotkeyId, mods, vk))
            {
                _instanceHotkeys[processId] = (hotkeyId, binding);
                return true;
            }

            AppLogger.Warn($"HotkeyService: no se pudo registrar hotkey de instancia pid={processId} ({binding})");
            return false;
        }

        public void UnregisterForInstance(uint processId)
        {
            if (_instanceHotkeys.TryGetValue(processId, out var info))
            {
                if (_hwnd != IntPtr.Zero)
                    User32.UnregisterHotKey(_hwnd, info.HotkeyId);
                _instanceHotkeys.Remove(processId);
            }
        }

        public void UnregisterAll()
        {
            if (_hwnd == IntPtr.Zero) return;

            foreach (var id in _globalHotkeys.Keys)
                User32.UnregisterHotKey(_hwnd, id);

            foreach (var (hotkeyId, _) in _instanceHotkeys.Values)
                User32.UnregisterHotKey(_hwnd, hotkeyId);
        }

        public void SetSuspended(bool suspended) => _suspended = suspended;

        public void Dispose()
        {
            UnregisterAll();
            _hwndSource?.Dispose();
        }

        private void ReRegisterAll()
        {
            foreach (var (id, binding) in _globalHotkeys)
            {
                uint mods = ModifiersToWin32(binding.Modifiers);
                uint vk   = (uint)KeyInterop.VirtualKeyFromKey(binding.Key);
                User32.RegisterHotKey(_hwnd, id, mods, vk);
            }
            foreach (var (pid, (hotkeyId, binding)) in _instanceHotkeys)
            {
                uint mods = ModifiersToWin32(binding.Modifiers);
                uint vk   = (uint)KeyInterop.VirtualKeyFromKey(binding.Key);
                User32.RegisterHotKey(_hwnd, hotkeyId, mods, vk);
            }
        }

        private IntPtr HwndHook(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
        {
            if (msg != User32.WM_HOTKEY) return IntPtr.Zero;
            if (_suspended) { handled = true; return IntPtr.Zero; }

            int id = wParam.ToInt32();

            if (id == NextHotkeyId)
            {
                NextRequested?.Invoke();
                handled = true;
            }
            else if (id == PreviousHotkeyId)
            {
                PreviousRequested?.Invoke();
                handled = true;
            }
            else if (id == PrimaryHotkeyId)
            {
                PrimaryRequested?.Invoke();
                handled = true;
            }
            else
            {
                var match = _instanceHotkeys.FirstOrDefault(kvp => kvp.Value.HotkeyId == id);
                if (match.Key != 0)
                {
                    InstanceActivationRequested?.Invoke(match.Key);
                    handled = true;
                }
            }

            return IntPtr.Zero;
        }

        private static uint ModifiersToWin32(ModifierKeys modifiers)
        {
            uint val = 0;
            if ((modifiers & ModifierKeys.Alt)     != 0) val |= User32.MOD_ALT;
            if ((modifiers & ModifierKeys.Control) != 0) val |= User32.MOD_CONTROL;
            if ((modifiers & ModifierKeys.Shift)   != 0) val |= User32.MOD_SHIFT;
            return val;
        }

        private static bool IsValidBinding(HotkeyBinding binding) => !binding.IsEmpty;
    }
}
