import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import path from "node:path";
import test from "node:test";

import {
  aliveProcessInstances,
  assertNoPreexistingProcessInstances,
  closeMainWindow,
  processInstanceKey,
  processForestInstances,
  sendCtrlC,
  terminateProcessInstance,
  waitForProcessInstance,
} from "./DevLifecycleProcessInstances.mjs";

function waitForOutput(child, marker, label, timeoutMilliseconds = 10_000) {
  return new Promise((resolve, reject) => {
    let output = "";
    const timeout = setTimeout(() => {
      cleanup();
      reject(new Error(`${label} did not emit ${marker}`));
    }, timeoutMilliseconds);
    const onData = (chunk) => {
      output += chunk.toString();
      if (output.includes(marker)) {
        cleanup();
        resolve();
      }
    };
    const onExit = (code) => {
      cleanup();
      reject(new Error(`${label} exited ${code} before emitting ${marker}`));
    };
    const cleanup = () => {
      clearTimeout(timeout);
      child.stdout.off("data", onData);
      child.off("exit", onExit);
    };
    child.stdout.on("data", onData);
    child.once("exit", onExit);
  });
}

test("a pre-existing application instance makes the gate fail closed before launch", () => {
  assert.throws(
    () =>
      assertNoPreexistingProcessInstances(
        process.execPath,
        path.basename(process.execPath),
      ),
    /pre-existing application process instance/i,
  );
});

test("an unresponsive exact window makes bounded close fail without hanging cleanup", async () => {
  const child = spawn(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      'Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing -TypeDefinition \'using System; using System.Drawing; using System.Threading; using System.Windows.Forms; public static class MyAlbunsHungWindowFixture { public static void Run() { var form = new Form { Text = "MyAlbuns unresponsive gate fixture", ShowInTaskbar = false, FormBorderStyle = FormBorderStyle.None, StartPosition = FormStartPosition.Manual, Location = new Point(-32000, -32000), Size = new Size(1, 1) }; form.Show(); Console.WriteLine("READY"); Console.Out.Flush(); Thread.Sleep(60000); } }\'; [MyAlbunsHungWindowFixture]::Run()',
    ],
    { stdio: ["ignore", "pipe", "pipe"] },
  );
  try {
    await waitForOutput(child, "READY", "Unresponsive window fixture");
    const instance = await waitForProcessInstance(
      child.pid,
      "Unresponsive window fixture",
    );
    const startedAt = Date.now();
    assert.throws(
      () => closeMainWindow(instance),
      /bounded WM_CLOSE delivery timed out or failed/i,
    );
    assert.ok(
      Date.now() - startedAt < 10_000,
      "the public close authority must remain bounded",
    );
    assert.deepEqual(aliveProcessInstances([instance]), [instance]);
    const exit = new Promise((resolve) => child.once("exit", resolve));
    assert.equal(terminateProcessInstance(instance), true);
    await exit;
  } finally {
    child.kill();
  }
});

test("an exact responsive window receives bounded native close", async () => {
  const child = spawn(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      'Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing -TypeDefinition \'using System; using System.Drawing; using System.Windows.Forms; public static class MyAlbunsResponsiveWindowFixture { public static void Run() { var form = new Form { Text = "MyAlbuns responsive gate fixture", ShowInTaskbar = false, FormBorderStyle = FormBorderStyle.None, StartPosition = FormStartPosition.Manual, Location = new Point(-32000, -32000), Size = new Size(1, 1) }; form.Shown += (_, __) => { Console.WriteLine("READY"); Console.Out.Flush(); }; form.FormClosing += (_, __) => { Console.WriteLine("CLOSING"); Console.Out.Flush(); }; Application.Run(form); } }\'; [MyAlbunsResponsiveWindowFixture]::Run()',
    ],
    { stdio: ["ignore", "pipe", "pipe"] },
  );
  try {
    await waitForOutput(child, "READY", "Responsive window fixture");
    const instance = await waitForProcessInstance(
      child.pid,
      "Responsive window fixture",
    );
    const closing = waitForOutput(
      child,
      "CLOSING",
      "Responsive window fixture",
    );
    const exit = new Promise((resolve) => child.once("exit", resolve));
    closeMainWindow(instance);
    await closing;
    await exit;
    assert.deepEqual(aliveProcessInstances([instance]), []);
  } finally {
    child.kill();
  }
});

test("a reused PID cannot satisfy liveness, tree-root, or close authority", async () => {
  const child = spawn(process.execPath, ["-e", "setInterval(() => {}, 1000)"], {
    windowsHide: true,
    stdio: "ignore",
  });
  try {
    const instance = await waitForProcessInstance(child.pid, "Test child");
    const reusedPid = {
      ...instance,
      creationTimeUtc: "1601-01-01T00:00:00.0000000Z",
    };

    assert.deepEqual(aliveProcessInstances([reusedPid]), []);
    assert.deepEqual(processForestInstances([reusedPid]), []);
    assert.throws(
      () => closeMainWindow(reusedPid),
      /process instance no longer matches/i,
    );
    assert.throws(
      () => sendCtrlC(reusedPid),
      /process instance no longer matches/i,
    );
    assert.equal(terminateProcessInstance(reusedPid), false);
    assert.deepEqual(aliveProcessInstances([instance]), [instance]);
    assert.ok(
      processForestInstances([instance]).some(
        (candidate) =>
          processInstanceKey(candidate) === processInstanceKey(instance),
      ),
      "the exact live instance remains an authoritative tree root",
    );
    const exit = new Promise((resolve) => child.once("exit", resolve));
    assert.equal(terminateProcessInstance(instance), true);
    await exit;
    assert.deepEqual(aliveProcessInstances([instance]), []);
  } finally {
    child.kill();
  }
});
