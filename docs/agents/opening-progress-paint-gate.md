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
isolated user-data directory. The Project Host and progress browser must survive.
Global and opening-dialog debugging use separate ports, through
`MYALBUNS_DEV_GLOBAL_WEBVIEW_DEBUG_PORT` and
`MYALBUNS_DEV_OPENING_DIALOG_WEBVIEW_DEBUG_PORT`.

Keep the Windows desktop available while running. The observer places only its
temporary progress dialog above other windows once, without taking focus. It
samples actual composed desktop pixels; a DOM screenshot does not prove that
the native window painted. It rejects sustained black content, requires the
completed progress bar to paint, and verifies Cache completion and Project UI
readiness. Inspect `completed.png` for the status, percentage and image count.

On 2026-09-13, the original shared-browser implementation reproduced an actual
WebView2 browser exit with `0xC0000005` during Cache cleanup/reopening. Injecting
the same browser-loss pattern left its progress surface black for 7.8 seconds.
Isolation alone also encountered an independent progress-browser access
violation. With GPU composition disabled only for the dedicated progress
environment, repeated fault-injection runs remained painted through `60 de 60`.
This proves failure containment; it does not identify the internal WebView2
instruction that caused the access violation.
