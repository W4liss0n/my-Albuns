function ConvertTo-WindowsProcessArgument {
    param([Parameter(Mandatory = $true)] [AllowEmptyString()] [string] $Argument)

    if ($Argument.Length -gt 0 -and $Argument -notmatch '[\s"]') {
        return $Argument
    }

    $quoted = [System.Text.StringBuilder]::new()
    [void] $quoted.Append('"')
    $backslashCount = 0
    foreach ($character in $Argument.ToCharArray()) {
        if ($character -eq '\') {
            $backslashCount += 1
            continue
        }
        if ($character -eq '"') {
            [void] $quoted.Append('\', (2 * $backslashCount) + 1)
            [void] $quoted.Append('"')
            $backslashCount = 0
            continue
        }
        if ($backslashCount -gt 0) {
            [void] $quoted.Append('\', $backslashCount)
            $backslashCount = 0
        }
        [void] $quoted.Append($character)
    }
    if ($backslashCount -gt 0) {
        [void] $quoted.Append('\', 2 * $backslashCount)
    }
    [void] $quoted.Append('"')
    return $quoted.ToString()
}
