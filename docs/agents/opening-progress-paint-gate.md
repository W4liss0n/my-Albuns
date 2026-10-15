# Opening progress native paint gate

Use `scripts/Run-OpeningProgressPaintGate.mjs` with a debug/custom-protocol
application, an isolated fixture Project with available originals, a new output
directory and its image count:

```powershell
node scripts/Run-OpeningProgressPaintGate.mjs <application.exe> <fixture.myalbuns> <new-output-directory> 60
node scripts/Run-OpeningProgressPaintGate.mjs <application.exe> <fixture.myalbuns> <another-new-directory> 60 --fail-owner-browser
```

The existing ignored Rust test
`product_runtime::tests::prepare_cache_reconstruction_fixture` creates the
60-image fixture under the absolute directory supplied through
`MYALBUNS_CACHE_RECONSTRUCTION_FIXTURE`.

The gate isolates application data, retains exact process identities and only
terminates its own application tree. The failure case additionally terminates
the hidden Global WebView2 browser after checking both its parent process and
isolated user-data directory, found through
`MYALBUNS_DEV_GLOBAL_WEBVIEW_DEBUG_PORT`. The Project Host and the progress
window must survive.

Since 2026-10-14 the opening progress is a native Win32 window
(`src-tauri/src/native_progress_window.rs`) with no WebView2 browser, so the
former `--fail-progress-browser` case no longer exists. Recovery and
External-copy decisions open as pages in the Global browser; the native gates
that drive them reach them through the Global debugging port.

Keep the Windows desktop available while running. The observer places only its
temporary progress dialog above other windows once, without taking focus. It
samples actual composed desktop pixels; a DOM screenshot does not prove that
the native window painted. It rejects sustained black content, requires the
measured progress bar to paint, and verifies Cache completion and Project UI
readiness. Inspect `progress.png` for the status, percentage and image count.
`completed.png` additionally records the final bar when it remains exposed long
enough for the desktop sampler. Completing the work and releasing the Project
must not require an artificial minimum dwell time at 100%.

On 2026-09-13, the original shared-browser implementation reproduced an actual
WebView2 browser exit with `0xC0000005` during Cache cleanup/reopening. Injecting
the same browser-loss pattern left its progress surface black for 7.8 seconds.
Isolation alone also encountered an independent progress-browser access
violation. With GPU composition disabled only for the dedicated progress
environment, repeated fault-injection runs remained painted through `60 de 60`.
This proves failure containment; it does not identify the internal WebView2
instruction that caused the access violation. The native progress window
replaced that dedicated progress browser on 2026-10-14.
