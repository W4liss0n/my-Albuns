import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { captureProcessInstance, powershellJson } from "./DevLifecycleProcessInstances.mjs";

// The caller supplies an already scoped browser identity, never just a PID.
export function startWebviewFailureCapture(procdump, browser, directory) {
  assert.deepEqual(captureProcessInstance(browser.processId), browser);
  const signedByMicrosoft = powershellJson(String.raw`
    $ErrorActionPreference = 'Stop'
    Import-Module (Join-Path $PSHOME 'Modules/Microsoft.PowerShell.Security/Microsoft.PowerShell.Security.psd1')
    $signature = Get-AuthenticodeSignature -LiteralPath $env:MYALBUNS_GATE_PROCDUMP
    [Console]::Out.Write((ConvertTo-Json ([bool]($signature.Status -eq 'Valid' -and $signature.SignerCertificate.Subject -match 'O=Microsoft Corporation'))))
  `, { MYALBUNS_GATE_PROCDUMP: procdump });
  assert.ok(signedByMicrosoft, "ProcDump must have a valid Microsoft signature");
  mkdirSync(directory);
  const monitor = spawn(procdump, ["-accepteula", "-mm", "-e", "-t", "-n", "1", String(browser.processId), directory],
    { windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
  const chunks = [];
  monitor.stdout.on("data", chunk => chunks.push(chunk));
  monitor.stderr.on("data", chunk => chunks.push(chunk));
  const completion = new Promise((resolve, reject) => {
    monitor.once("error", reject);
    monitor.once("exit", code => {
      const buffer = Buffer.concat(chunks);
      const log = buffer.includes(0) ? buffer.toString("utf16le") : buffer.toString("utf8");
      writeFileSync(`${directory}/procdump.log`, log);
      writeFileSync(`${directory}/process.json`, JSON.stringify({ browser, exitCode: code }, null, 2));
      resolve(code);
    });
  });
  return {
    async ready() {
      const deadline = Date.now() + 5000;
      while (Date.now() < deadline) {
        const buffer = Buffer.concat(chunks);
        const log = buffer.includes(0) ? buffer.toString("utf16le") : buffer.toString("utf8");
        if (log.includes("Press Ctrl-C to end monitoring")) return;
        assert.equal(monitor.exitCode, null, log);
        await new Promise(resolve => setTimeout(resolve, 50));
      }
      throw new Error("ProcDump did not attach to the test browser");
    },
    async stop() {
      if (monitor.exitCode === null) {
        const current = captureProcessInstance(browser.processId);
        if (current?.creationTimeUtc === browser.creationTimeUtc) {
          const cancel = spawn(procdump, ["-cancel", String(browser.processId)], { windowsHide: true, stdio: "ignore" });
          await new Promise((resolve, reject) => { cancel.once("exit", resolve); cancel.once("error", reject); });
        }
      }
      return completion;
    },
  };
}
