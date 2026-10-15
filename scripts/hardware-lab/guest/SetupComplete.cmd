@echo off
rem Runs once as SYSTEM at the end of Windows setup inside the lab VM.
rem PowerShell Direct sessions of the local admin get the full token.
reg add HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System /v LocalAccountTokenFilterPolicy /t REG_DWORD /d 1 /f
rem A locked or sleeping console hides the app window and stops WebView2 frames.
reg add HKLM\SOFTWARE\Policies\Microsoft\Windows\Personalization /v NoLockScreen /t REG_DWORD /d 1 /f
powercfg /change monitor-timeout-ac 0
powercfg /change standby-timeout-ac 0
powercfg /change disk-timeout-ac 0
powercfg /hibernate off
rem The data disk keeps the drive letter of the host folders copied into it.
powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-Volume -FileSystemLabel LabData | Get-Partition | Set-Partition -NewDriveLetter D"
