# Validation operating policy

Use `npm run validate` for headless validation during interactive work. It first
builds the imaging processor required by Tauri, including on a fresh checkout, then runs
frontend build/contracts/types, frontend tests, automation tests, Rust quality
and Rust tests serially, retaining logs and a report in `.tools/validation/`.
Focused test commands remain available while editing. The headless default
never launches MyAlbuns or a Win32 window fixture.

`npm run ui:acceptance` already uses a headless browser. Set
`MYALBUNS_UI_SCENARIO_IDS` to the affected scenarios before capturing them.
Browser captures prove frontend behavior and appearance, not WebView2/native
ownership or physical GPU compatibility. Visual acceptance still requires review
of the captured evidence; a successful capture alone is not visual approval.

## Native scenarios

Visible native gates are disabled locally unless the user gives explicit
permission for that run and the command receives `-AllowVisibleWindows`.
Use an isolated Windows environment with verified hardware WebGL2 for native
acceptance. Do not work around this policy by invoking a JavaScript runner directly. Do not use a
self-hosted runner on the user's desktop without their explicit permission.
The previous automatic focused run after GREEN is superseded.

Prepare the binary once with `npm run build:native-tests`. This builds the
Tauri debug `custom-protocol` application, processor and fixture generator,
and writes `.tools/native-gate-build.json`. It requires clean committed source.
The focused gate reuses that manifest, validating the source commit and every
executable hash before launch. A different commit, dirty source or changed
binary requires a new build; a plain Cargo executable is not a substitute.

`npm run test:native-owned-dialogs -- -Scenario <id>` runs one fresh fixture:

- `external-copy-opening-owner`: native Global ownership, exact pending Host,
  native picker ownership and correlated cancellation/activation terminals.
- `late-graphics-project-dialog`: Project ownership, inert workspace and the
  exact graphics failure close action.

Omitting `-Scenario` selects both, serially, with the same prepared build.
Each selected scenario must pass its own evidence requirements. Unselected
scenarios are absent from the receipt and are never reported as passed.
A failed run retains window diagnostics, screenshots where available, process
logs and driver traces before cleanup. Inspect those artifacts before rerunning
only the failed scenario. Do not add automatic retries to turn failures green.

PR updates run headless validation automatically. The native CI pilot is manual:
supply `native_runner` with a configured hosted Windows x64 runner label that has
verified hardware WebGL2. An empty input runs headless validation only. Manual
dispatch requires this workflow on the default branch; do not merge unresolved
product work just to enable it.

The pilot on standard `windows-2022` at `f83304f` reached the application's safe
mode: hardware WebGL2 could not be created, so project opening was correctly
blocked. Its [retained diagnostics](https://github.com/W4liss0n/my-Albuns/actions/runs/33935287054)
include the startup page text and screenshot. The pilot is not approved. Do not
repeat it on that environment without a relevant change, or bypass the product's
graphics policy to obtain a passing result.

When selected, headless validation and the native pilot run as separate, parallel
jobs with separate artifacts. A skipped native job provides no native acceptance
evidence. Rerun only the failed job when its environment or code has changed.
Either failure keeps the workflow failed; neither job suppresses the other. A
native build that detects source changes retains changed paths and source
snapshots before refusing to run the scenario. Other scenarios remain explicit
until their hosted behavior is verified. Win32 probe fixtures are opt-in with
`MYALBUNS_NATIVE_PROBE_TESTS=1`, only in that isolated native environment or during
an explicitly authorized local native run.

## Full journey and existing evidence

The full productive journey remains a separate integration/release check with
explicit permission. It is not part of daily validation or the focused CI pilot.
The session-recovery and Save As aliases still invoke that same full legacy
journey; never run all three as if they were independent suites. Further
migration of its segments must preserve each public assertion and proof layer.

The unresolved clean-close failure found in PR #64 remains unvalidated.
Passing the headless command or the CI pilot does not close that finding or
approve the full productive journey. Historical evidence retains its original
commit attribution. A PR with this pending failure must not be declared fully
validated or merged on the strength of the pilot alone.
