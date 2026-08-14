param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('select', 'cancel')]
    [string] $Action,

    [Parameter(Mandatory = $true)]
    [int] $ProcessId,

    [Parameter(Mandatory = $true)]
    [string] $CreationTimeUtc,

    [Parameter(Mandatory = $true)]
    [string] $DialogTitle,

    [string] $DestinationPath,

    [ValidateRange(1, 120)]
    [int] $TimeoutSeconds = 30
)

$ErrorActionPreference = 'Stop'

if ($Action -eq 'select' -and [string]::IsNullOrWhiteSpace($DestinationPath)) {
    throw 'DestinationPath is required when selecting a native save destination.'
}

$nativeWindowSource = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class MyAlbunsNativeDialogWindow
{
    private const uint GW_OWNER = 4;

    private delegate bool EnumWindowsCallback(IntPtr window, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsCallback callback, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern bool EnumChildWindows(
        IntPtr parent,
        EnumWindowsCallback callback,
        IntPtr parameter
    );

    [DllImport("user32.dll")]
    private static extern IntPtr GetWindow(IntPtr window, uint command);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);

    [DllImport("user32.dll")]
    private static extern bool IsWindow(IntPtr window);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr window);

    [DllImport("user32.dll")]
    private static extern int GetDlgCtrlID(IntPtr window);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern IntPtr SendMessageTimeout(
        IntPtr window,
        uint message,
        UIntPtr wParam,
        IntPtr lParam,
        uint flags,
        uint timeoutMilliseconds,
        out UIntPtr result
    );

    [DllImport(
        "user32.dll",
        EntryPoint = "SendMessageTimeoutW",
        CharSet = CharSet.Unicode,
        SetLastError = true
    )]
    private static extern IntPtr SendMessageTimeoutText(
        IntPtr window,
        uint message,
        UIntPtr wParam,
        string lParam,
        uint flags,
        uint timeoutMilliseconds,
        out UIntPtr result
    );

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetClassName(IntPtr window, StringBuilder className, int capacity);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowText(IntPtr window, StringBuilder title, int capacity);

    public static IntPtr[] FindOwnedFileDialogs(uint processId, string expectedTitle)
    {
        var matches = new List<IntPtr>();
        EnumWindows((window, _) =>
        {
            if (Matches(window, processId, expectedTitle))
            {
                matches.Add(window);
            }
            return true;
        }, IntPtr.Zero);
        return matches.ToArray();
    }

    public static bool Matches(IntPtr window, uint processId, string expectedTitle)
    {
        if (!IsWindow(window) || !IsWindowVisible(window))
        {
            return false;
        }

        uint observedProcessId;
        GetWindowThreadProcessId(window, out observedProcessId);
        if (observedProcessId != processId)
        {
            return false;
        }

        var className = new StringBuilder(64);
        GetClassName(window, className, className.Capacity);
        if (!String.Equals(className.ToString(), "#32770", StringComparison.Ordinal))
        {
            return false;
        }

        var title = new StringBuilder(512);
        GetWindowText(window, title, title.Capacity);
        if (!String.Equals(title.ToString(), expectedTitle, StringComparison.Ordinal))
        {
            return false;
        }

        var owner = GetWindow(window, GW_OWNER);
        if (owner == IntPtr.Zero || !IsWindow(owner))
        {
            return false;
        }
        uint ownerProcessId;
        GetWindowThreadProcessId(owner, out ownerProcessId);
        return ownerProcessId == processId;
    }

    public static bool ClickDialogButton(IntPtr dialog, uint processId, int controlId)
    {
        const uint BM_CLICK = 0x00F5;
        const uint SMTO_BLOCK = 0x0001;
        const uint SMTO_ABORTIFHUNG = 0x0002;

        var button = FindDialogControl(dialog, processId, controlId, "Button");
        if (button == IntPtr.Zero)
        {
            return false;
        }

        UIntPtr result;
        return SendMessageTimeout(
            button,
            BM_CLICK,
            UIntPtr.Zero,
            IntPtr.Zero,
            SMTO_BLOCK | SMTO_ABORTIFHUNG,
            5000,
            out result
        ) != IntPtr.Zero;
    }

    public static bool SetFileName(IntPtr dialog, uint processId, string text)
    {
        const uint WM_SETTEXT = 0x000C;
        const uint SMTO_BLOCK = 0x0001;
        const uint SMTO_ABORTIFHUNG = 0x0002;

        var edit = FindDialogControl(dialog, processId, 1001, "Edit");
        if (edit == IntPtr.Zero)
        {
            return false;
        }

        UIntPtr result;
        return SendMessageTimeoutText(
            edit,
            WM_SETTEXT,
            UIntPtr.Zero,
            text,
            SMTO_BLOCK | SMTO_ABORTIFHUNG,
            5000,
            out result
        ) != IntPtr.Zero;
    }

    private static IntPtr FindDialogControl(
        IntPtr dialog,
        uint processId,
        int controlId,
        string expectedClass
    )
    {
        var matches = new List<IntPtr>();
        EnumChildWindows(dialog, (window, _) =>
        {
            uint observedProcessId;
            GetWindowThreadProcessId(window, out observedProcessId);
            if (observedProcessId != processId || GetDlgCtrlID(window) != controlId)
            {
                return true;
            }
            var className = new StringBuilder(64);
            GetClassName(window, className, className.Capacity);
            if (String.Equals(className.ToString(), expectedClass, StringComparison.Ordinal))
            {
                matches.Add(window);
            }
            return true;
        }, IntPtr.Zero);
        return matches.Count == 1 ? matches[0] : IntPtr.Zero;
    }

}
'@
Add-Type -TypeDefinition $nativeWindowSource

function Open-ExactProcess {
    $process = Get-Process -Id $ProcessId -ErrorAction Stop
    [void] $process.Handle
    if ($process.HasExited) {
        $process.Dispose()
        throw 'The application process exited before native dialog automation.'
    }
    $observed = Get-CimInstance `
        Win32_Process `
        -Filter "ProcessId = $ProcessId" `
        -ErrorAction Stop
    if (
        $null -eq $observed -or
        $observed.CreationDate.ToUniversalTime().ToString('O') -cne $CreationTimeUtc
    ) {
        $process.Dispose()
        throw 'The native dialog no longer belongs to the expected process instance.'
    }
    return $process
}

function Assert-ExactProcess([System.Diagnostics.Process] $Process) {
    if ($Process.HasExited) {
        throw 'The application process exited during native dialog automation.'
    }
    $observed = Get-CimInstance `
        Win32_Process `
        -Filter "ProcessId = $ProcessId" `
        -ErrorAction Stop
    if (
        $null -eq $observed -or
        $observed.CreationDate.ToUniversalTime().ToString('O') -cne $CreationTimeUtc
    ) {
        throw 'The native dialog process instance changed during automation.'
    }
}

function Find-Dialog {
    $handles = [MyAlbunsNativeDialogWindow]::FindOwnedFileDialogs(
        [uint32] $ProcessId,
        $DialogTitle
    )
    if ($handles.Count -gt 1) {
        throw 'More than one native save dialog matched the exact application instance.'
    }
    if ($handles.Count -eq 0) {
        return $null
    }
    return [pscustomobject]@{ Handle = $handles[0] }
}

function Assert-DialogOwner([IntPtr] $Handle) {
    Assert-ExactProcess $process
    if (
        -not [MyAlbunsNativeDialogWindow]::Matches(
            $Handle,
            [uint32] $ProcessId,
            $DialogTitle
        )
    ) {
        throw 'The native save dialog no longer belongs to the expected application instance.'
    }
}

$process = Open-ExactProcess
try {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    $dialog = $null
    while ($null -eq $dialog -and [DateTime]::UtcNow -lt $deadline) {
        Assert-ExactProcess $process
        $dialog = Find-Dialog
        if ($null -eq $dialog) {
            Start-Sleep -Milliseconds 50
        }
    }
    if ($null -eq $dialog) {
        throw 'The expected native save dialog did not become observable.'
    }

    Assert-DialogOwner $dialog.Handle
    if ($Action -eq 'select') {
        if (
            -not [MyAlbunsNativeDialogWindow]::SetFileName(
                $dialog.Handle,
                [uint32] $ProcessId,
                [System.IO.Path]::GetFullPath($DestinationPath)
            )
        ) {
            throw 'The exact native save dialog exposed no bounded file-name control.'
        }
        Assert-DialogOwner $dialog.Handle
    }
    Assert-DialogOwner $dialog.Handle
    $buttonId = if ($Action -eq 'select') { 1 } else { 2 }
    if (
        -not [MyAlbunsNativeDialogWindow]::ClickDialogButton(
            $dialog.Handle,
            [uint32] $ProcessId,
            $buttonId
        )
    ) {
        throw 'The exact native save dialog exposed no bounded action control.'
    }

    $closeDeadline = [DateTime]::UtcNow.AddSeconds(10)
    while (
        [MyAlbunsNativeDialogWindow]::Matches(
            $dialog.Handle,
            [uint32] $ProcessId,
            $DialogTitle
        ) -and
        [DateTime]::UtcNow -lt $closeDeadline
    ) {
        Assert-ExactProcess $process
        Start-Sleep -Milliseconds 25
    }
    if (
        [MyAlbunsNativeDialogWindow]::Matches(
            $dialog.Handle,
            [uint32] $ProcessId,
            $DialogTitle
        )
    ) {
        throw 'The requested native dialog action did not close the exact dialog.'
    }

    [ordered]@{
        action = $Action
        dialogTitle = $DialogTitle
        exactProcess = $true
    } | ConvertTo-Json -Compress
}
finally {
    $process.Dispose()
}
