import { spawn } from "node:child_process";
import net from "node:net";

import {
  aliveProcessInstances,
  terminateProcessInstance,
  waitForProcessInstance,
} from "./DevLifecycleProcessInstances.mjs";

const reservedTcpPorts = new Set();
const delay = (milliseconds) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

export async function disposeConfirmedWebDriver(driver) {
  await driver.dispose();
  return undefined;
}

export async function findFreeTcpPort() {
  while (true) {
    const port = await probeTcpPort(0);
    if (!reservedTcpPorts.has(port)) {
      reservedTcpPorts.add(port);
      return port;
    }
  }
}

export async function findFreeTcpPortInRange(minimum, maximum) {
  if (
    !Number.isInteger(minimum) ||
    !Number.isInteger(maximum) ||
    minimum < 1 ||
    maximum > 65_535 ||
    minimum > maximum
  ) {
    throw new RangeError("the TCP port range is invalid");
  }
  for (let port = minimum; port <= maximum; ++port) {
    if (reservedTcpPorts.has(port)) continue;
    try {
      await probeTcpPort(port);
      reservedTcpPorts.add(port);
      return port;
    } catch (error) {
      if (error?.code !== "EADDRINUSE" && error?.code !== "EACCES") {
        throw error;
      }
    }
  }
  throw new Error(`no TCP port is available in ${minimum}-${maximum}`);
}

export async function waitForHttp(url, label, timeoutMilliseconds = 30_000) {
  const deadline = Date.now() + timeoutMilliseconds;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url, {
        signal: AbortSignal.timeout(1_000),
      });
      if (response.ok) return;
      lastError = new Error(`${label} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await delay(50);
  }
  throw new Error(
    `${label} did not become ready: ${lastError ?? "unknown error"}`,
    { cause: lastError },
  );
}

export async function webViewDevToolsTargets(debugPort, label) {
  await waitForHttp(
    `http://127.0.0.1:${debugPort}/json/list`,
    `${label} DevTools targets`,
  );
  const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
  return response.json();
}

export async function attachWebView2Driver({
  debugPort,
  driverTerminationTimeoutMilliseconds = 30_000,
  label,
  nativeDriverPath,
  projectDialogDebugPort,
  sessionTimeoutMilliseconds = 60_000,
  workingDirectory,
}) {
  await waitForHttp(
    `http://127.0.0.1:${debugPort}/json/version`,
    `${label} DevTools endpoint`,
  );
  const driverPort = await findFreeTcpPort();
  const child = spawn(
    nativeDriverPath,
    [`--port=${driverPort}`, "--host=127.0.0.1"],
    {
      cwd: workingDirectory,
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  let output = "";
  child.stdout.on("data", (chunk) => {
    output += chunk.toString();
  });
  child.stderr.on("data", (chunk) => {
    output += chunk.toString();
  });
  const instance = await waitForProcessInstance(child.pid, `${label} WebDriver`);
  const baseUrl = `http://127.0.0.1:${driverPort}`;
  await waitForHttp(`${baseUrl}/status`, `${label} WebDriver`);
  const rawRequest = createWebDriverClient(baseUrl);
  const request = async (method, endpoint, body, timeout) => {
    try {
      return await rawRequest(method, endpoint, body, timeout);
    } catch (error) {
      throw new Error(
        `${label} WebDriver ${method} ${endpoint} failed; driverExitCode=${child.exitCode}; output=${output.slice(-1_000)}`,
        { cause: error },
      );
    }
  };
  const session = await request(
    "POST",
    "/session",
    {
      capabilities: {
        alwaysMatch: {
          browserName: "webview2",
          pageLoadStrategy: "none",
          "ms:edgeChromium": true,
          "ms:edgeOptions": {
            debuggerAddress: `127.0.0.1:${debugPort}`,
          },
        },
      },
    },
    sessionTimeoutMilliseconds,
  );
  if (!session.sessionId) {
    throw new Error(`${label} WebDriver returned no session id`);
  }
  const sessionId = session.sessionId;
  await request("POST", `/session/${sessionId}/timeouts`, {
    implicit: 250,
    pageLoad: 5_000,
    script: 5_000,
  });
  return {
    projectDialogDebugPort,
    request,
    sessionId,
    async dispose() {
      try {
        await request("DELETE", `/session/${sessionId}`);
      } catch {
        // The WebView can close before the attach-only session is deleted.
      }
      terminateProcessInstance(instance);
      const deadline = Date.now() + driverTerminationTimeoutMilliseconds;
      while (
        aliveProcessInstances([instance]).length !== 0 &&
        Date.now() < deadline
      ) {
        await delay(25);
      }
      if (aliveProcessInstances([instance]).length !== 0) {
        throw new Error(`${label} WebDriver did not terminate`);
      }
      while (
        child.exitCode === null &&
        child.signalCode === null &&
        Date.now() < deadline
      ) {
        await delay(25);
      }
      if (child.exitCode === null && child.signalCode === null) {
        throw new Error(`${label} WebDriver process handle did not terminate`);
      }
      return output;
    },
  };
}

export async function switchToWebDriverWindow(driver, predicate, label) {
  const handles = await driver.request(
    "GET",
    `/session/${driver.sessionId}/window/handles`,
  );
  for (const handle of handles) {
    await driver.request("POST", `/session/${driver.sessionId}/window`, {
      handle,
    });
    const url = await driver.request(
      "POST",
      `/session/${driver.sessionId}/execute/sync`,
      { script: "return window.location.href;", args: [] },
    );
    if (predicate(url)) return { handle, url };
  }
  throw new Error(`${label} was not found among ${handles.length} WebViews`);
}

function probeTcpPort(port) {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => {
      const address = server.address();
      server.close((error) => {
        if (error) reject(error);
        else resolve(address.port);
      });
    });
  });
}

export function createWebDriverClient(
  baseUrl,
  { defaultTimeoutMilliseconds = 10_000 } = {},
) {
  return async (
    method,
    endpoint,
    body,
    timeoutMilliseconds = defaultTimeoutMilliseconds,
  ) => {
    const requestBody = body === undefined && method === "POST" ? {} : body;
    const response = await fetch(`${baseUrl}${endpoint}`, {
      method,
      headers:
        requestBody === undefined
          ? undefined
          : { "content-type": "application/json" },
      body: requestBody === undefined ? undefined : JSON.stringify(requestBody),
      signal: AbortSignal.timeout(timeoutMilliseconds),
    });
    const text = await response.text();
    const payload = text ? JSON.parse(text) : { value: null };
    if (!response.ok || payload.value?.error) {
      throw new Error(
        `${method} ${endpoint} failed (${response.status}): ${JSON.stringify(payload)}`,
      );
    }
    return payload.value;
  };
}
