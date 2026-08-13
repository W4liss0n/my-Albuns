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
