import { powershellJson } from "./DevLifecycleProcessInstances.mjs";

export function nativeWindowTitle(instance) {
  const encoded = powershellJson(
    String.raw`
$observed = Get-CimInstance Win32_Process -Filter "ProcessId = $env:MYALBUNS_GATE_WINDOW_PID" -ErrorAction Stop
if ($null -eq $observed -or $observed.CreationDate.ToUniversalTime().ToString('O') -cne $env:MYALBUNS_GATE_WINDOW_CREATED) {
    throw 'The native window no longer belongs to the expected process instance.'
}
$process = Get-Process -Id ([int]$env:MYALBUNS_GATE_WINDOW_PID) -ErrorAction Stop
[void]$process.Handle
if ($process.HasExited) {
    throw 'The native window process exited before its title was observed.'
}
$titleBytes = [System.Text.Encoding]::UTF8.GetBytes([string]$process.MainWindowTitle)
$titleBase64 = [System.Convert]::ToBase64String($titleBytes)
[Console]::Out.Write((ConvertTo-Json -InputObject $titleBase64 -Compress))
`,
    {
      MYALBUNS_GATE_WINDOW_PID: String(instance.processId),
      MYALBUNS_GATE_WINDOW_CREATED: instance.creationTimeUtc,
    },
  );
  return Buffer.from(encoded, "base64").toString("utf8");
}

export function nativeOwnedWindowState(instance) {
  const observed =
    powershellJson(
      String.raw`
$ErrorActionPreference = 'Stop'
$process = Get-CimInstance Win32_Process -Filter "ProcessId = $env:MYALBUNS_GATE_WINDOW_PID" -ErrorAction Stop
if ($null -eq $process -or $process.CreationDate.ToUniversalTime().ToString('O') -cne $env:MYALBUNS_GATE_WINDOW_CREATED) {
    throw 'The native windows no longer belong to the expected process instance.'
}
if (-not ('MyAlbunsGate.NativeWindowProbe' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

namespace MyAlbunsGate {
    public sealed class NativeWindowSnapshot {
        public long Hwnd { get; set; }
        public long OwnerHwnd { get; set; }
        public bool Visible { get; set; }
        public bool Enabled { get; set; }
        public string Title { get; set; }
    }

    public static class NativeWindowProbe {
        private delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr parameter);

        [StructLayout(LayoutKind.Sequential)]
        private struct NativeRectangle {
            public int Left;
            public int Top;
            public int Right;
            public int Bottom;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct GuiThreadInfo {
            public uint Size;
            public uint Flags;
            public IntPtr ActiveWindow;
            public IntPtr FocusWindow;
            public IntPtr CaptureWindow;
            public IntPtr MenuOwnerWindow;
            public IntPtr MoveSizeWindow;
            public IntPtr CaretWindow;
            public NativeRectangle CaretRectangle;
        }

        [DllImport("user32.dll")]
        private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);

        [DllImport("user32.dll")]
        private static extern bool EnumThreadWindows(
            uint threadId,
            EnumWindowsProc callback,
            IntPtr parameter
        );

        [DllImport("user32.dll")]
        private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);

        [DllImport("user32.dll")]
        private static extern IntPtr GetWindow(IntPtr hwnd, uint command);

        [DllImport("user32.dll")]
        private static extern IntPtr GetAncestor(IntPtr hwnd, uint flags);

        [DllImport("user32.dll")]
        private static extern bool GetGUIThreadInfo(
            uint threadId,
            ref GuiThreadInfo information
        );

        [DllImport("user32.dll")]
        private static extern bool IsWindow(IntPtr hwnd);

        [DllImport("user32.dll")]
        private static extern bool IsWindowVisible(IntPtr hwnd);

        [DllImport("user32.dll")]
        private static extern bool IsWindowEnabled(IntPtr hwnd);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int maximum);

        private static void ObserveOwnerChain(
            uint expectedProcessId,
            IntPtr seed,
            List<NativeWindowSnapshot> windows,
            HashSet<long> observedHandles
        ) {
            var pending = new Queue<IntPtr>();
            var traversedHandles = new HashSet<long>();
            pending.Enqueue(seed);
            while (pending.Count > 0) {
                var hwnd = pending.Dequeue();
                if (hwnd == IntPtr.Zero || !IsWindow(hwnd)) continue;
                if (!traversedHandles.Add(hwnd.ToInt64())) continue;

                var owner = GetWindow(hwnd, 4);
                var rootOwner = GetAncestor(hwnd, 3);
                if (owner != IntPtr.Zero) pending.Enqueue(owner);
                if (rootOwner != IntPtr.Zero && rootOwner != hwnd) {
                    pending.Enqueue(rootOwner);
                }

                uint processId;
                GetWindowThreadProcessId(hwnd, out processId);
                if (processId != expectedProcessId ||
                    !observedHandles.Add(hwnd.ToInt64())) continue;
                var text = new StringBuilder(512);
                GetWindowText(hwnd, text, text.Capacity);
                windows.Add(new NativeWindowSnapshot {
                    Hwnd = hwnd.ToInt64(),
                    OwnerHwnd = owner.ToInt64(),
                    Visible = IsWindowVisible(hwnd),
                    Enabled = IsWindowEnabled(hwnd),
                    Title = text.ToString(),
                });
            }
        }

        public static NativeWindowSnapshot[] Snapshot(uint expectedProcessId) {
            var windows = new List<NativeWindowSnapshot>();
            var handles = new HashSet<long>();
            EnumWindows((hwnd, _) => {
                ObserveOwnerChain(expectedProcessId, hwnd, windows, handles);
                return true;
            }, IntPtr.Zero);
            using (var process = Process.GetProcessById((int)expectedProcessId)) {
                process.Refresh();
                ObserveOwnerChain(
                    expectedProcessId,
                    process.MainWindowHandle,
                    windows,
                    handles
                );
                foreach (ProcessThread thread in process.Threads) {
                    EnumThreadWindows((uint)thread.Id, (hwnd, _) => {
                        ObserveOwnerChain(expectedProcessId, hwnd, windows, handles);
                        return true;
                    }, IntPtr.Zero);
                    var information = new GuiThreadInfo {
                        Size = (uint)Marshal.SizeOf(typeof(GuiThreadInfo)),
                    };
                    if (GetGUIThreadInfo((uint)thread.Id, ref information)) {
                        foreach (var hwnd in new[] {
                            information.ActiveWindow,
                            information.FocusWindow,
                            information.CaptureWindow,
                            information.MenuOwnerWindow,
                            information.MoveSizeWindow,
                            information.CaretWindow,
                        }) {
                            ObserveOwnerChain(
                                expectedProcessId,
                                hwnd,
                                windows,
                                handles
                            );
                        }
                    }
                }
            }
            windows.Sort((left, right) => left.Hwnd.CompareTo(right.Hwnd));
            return windows.ToArray();
        }
    }
}
'@
}
$windows = @([MyAlbunsGate.NativeWindowProbe]::Snapshot([uint32]$env:MYALBUNS_GATE_WINDOW_PID) | ForEach-Object {
    [ordered]@{
        hwnd = [long]$_.Hwnd
        ownerHwnd = [long]$_.OwnerHwnd
        visible = [bool]$_.Visible
        enabled = [bool]$_.Enabled
        title = [string]$_.Title
    }
})
[Console]::Out.Write((ConvertTo-Json -InputObject $windows -Compress))
`,
      {
        MYALBUNS_GATE_WINDOW_PID: String(instance.processId),
        MYALBUNS_GATE_WINDOW_CREATED: instance.creationTimeUtc,
      },
    ) ?? [];
  const windows = Array.isArray(observed) ? observed : [observed];
  const visibleOwnedWindows = windows.filter(
    (window) => window.visible && window.ownerHwnd !== 0,
  );
  const dialog = visibleOwnedWindows.length === 1 ? visibleOwnedWindows[0] : null;
  const owner = dialog
    ? windows.find((window) => window.hwnd === dialog.ownerHwnd) ?? null
    : null;
  return {
    dialogCount: visibleOwnedWindows.length,
    dialog,
    owner,
    windows,
  };
}
