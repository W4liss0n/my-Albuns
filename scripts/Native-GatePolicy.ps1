function Assert-NativeGateExecutionAllowed {
    param([switch] $AllowVisibleWindows)

    if ($AllowVisibleWindows -or (
            $env:GITHUB_ACTIONS -eq 'true' -and
            $env:RUNNER_ENVIRONMENT -eq 'github-hosted'
        )) {
        return
    }
    throw 'Visible native tests are disabled locally. Use the Windows CI workflow, or explicitly pass -AllowVisibleWindows when the desktop is available. The default validation is npm run validate.'
}
