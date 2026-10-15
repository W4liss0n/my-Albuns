# Started by a scheduled task inside the interactive session: window messages
# cannot cross from the PowerShell Direct session into the desktop session.
# Closing the windows lets the app end its Projects cleanly, so a repeated run
# does not open into recovery.
foreach ($process in @(Get-Process -Name 'myalbuns-desktop' -ErrorAction SilentlyContinue)) {
    if ($process.MainWindowHandle -ne [IntPtr]::Zero) {
        [void] $process.CloseMainWindow()
    }
}
