param([uint32]$TargetProcessId, [string]$OutputPath, [int]$DurationMilliseconds = 30000)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Imaging;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;

public class OpeningPaintSample {
    public long Milliseconds;
    public int Width, Height;
    public double BlackRatio;
    public bool Exposed;
}
public static class OpeningProgressPaint {
    delegate bool Callback(IntPtr handle, IntPtr unused);
    [StructLayout(LayoutKind.Sequential)] struct Rect { public int Left,Top,Right,Bottom; }
    [StructLayout(LayoutKind.Sequential)] struct Point { public int X,Y; }
    [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback, IntPtr unused);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr handle, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr handle);
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr handle, out Rect rect);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr handle, ref Point point);
    [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(Point point);
    [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr handle, uint flags);
    [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr handle, IntPtr after, int x, int y, int cx, int cy, uint flags);

    public static List<OpeningPaintSample> Observe(uint pid, string output, int duration) {
        var samples = new List<OpeningPaintSample>();
        var promoted = new HashSet<IntPtr>();
        var timer = Stopwatch.StartNew();
        bool savedPaint = false, savedBlack = false;
        File.WriteAllText(Path.Combine(output, "observer.ready"), "ready");
        while (timer.ElapsedMilliseconds < duration) {
            try { using (var process = Process.GetProcessById((int)pid)) { if (process.HasExited) break; } }
            catch (ArgumentException) { break; }
            EnumWindows((handle, unused) => {
                uint actual;
                GetWindowThreadProcessId(handle, out actual);
                if (actual != pid || !IsWindowVisible(handle)) return true;
                Rect rect;
                GetClientRect(handle, out rect);
                int width = rect.Right, height = rect.Bottom;
                if (width < 330 || width > 540 || height < 130 || height > 270) return true;
                // Keep only this temporary test dialog exposed, without stealing focus.
                // Do it once: repeated repositioning could mask a rendering failure.
                if (promoted.Add(handle)) SetWindowPos(handle, new IntPtr(-1), 0, 0, 0, 0, 0x0013);
                var origin = new Point();
                ClientToScreen(handle, ref origin);
                var center = new Point { X = origin.X + width / 2, Y = origin.Y + height / 2 };
                bool exposed = GetAncestor(WindowFromPoint(center), 2) == handle;
                var sample = new OpeningPaintSample { Milliseconds = timer.ElapsedMilliseconds,
                    Width = width, Height = height, Exposed = exposed };
                samples.Add(sample);
                if (!exposed) return true;
                using (var bitmap = new Bitmap(width - 8, height - 8)) {
                    using (var graphics = Graphics.FromImage(bitmap))
                        graphics.CopyFromScreen(origin.X + 4, origin.Y + 4, 0, 0, bitmap.Size);
                    int black = 0, total = 0, longestBlueRun = 0;
                    for (int y = 4; y < bitmap.Height; y += 8)
                        for (int x = 4; x < bitmap.Width; x += 8) {
                            var color = bitmap.GetPixel(x, y);
                            total++;
                            if (color.R < 20 && color.G < 20 && color.B < 20) black++;
                        }
                    // A long continuous blue run distinguishes completion from the
                    // short indeterminate indicator, title text and a blank window.
                    for (int y = bitmap.Height / 2; y < bitmap.Height - 15; y += 2) {
                        int run = 0;
                        for (int x = 0; x < bitmap.Width; x++) {
                            var color = bitmap.GetPixel(x, y);
                            run = color.B > 100 && color.B > color.R * 1.3 && color.B > color.G * 1.1 ? run + 1 : 0;
                            longestBlueRun = Math.Max(longestBlueRun, run);
                        }
                    }
                    sample.BlackRatio = (double)black / Math.Max(1, total);
                    if (sample.BlackRatio > 0.96 && !savedBlack) {
                        bitmap.Save(Path.Combine(output, "black.png"), ImageFormat.Png);
                        savedBlack = true;
                    }
                    if (sample.BlackRatio < 0.2) {
                        if (!savedPaint) {
                            bitmap.Save(Path.Combine(output, "painted.png"), ImageFormat.Png);
                            savedPaint = true;
                        }
                        if (longestBlueRun >= (width - 40) * 0.99)
                            bitmap.Save(Path.Combine(output, "completed.png"), ImageFormat.Png);
                    }
                }
                return true;
            }, IntPtr.Zero);
            System.Threading.Thread.Sleep(50);
        }
        return samples;
    }
}
'@
[IO.Directory]::CreateDirectory($OutputPath) | Out-Null
$samples = [OpeningProgressPaint]::Observe($TargetProcessId, $OutputPath, $DurationMilliseconds)
$samples | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $OutputPath 'paint-samples.json') -Encoding UTF8
