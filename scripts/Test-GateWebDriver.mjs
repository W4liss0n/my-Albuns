import assert from "node:assert/strict";
import http from "node:http";
import test from "node:test";

import {
  createWebDriverClient,
  disposeConfirmedWebDriver,
  findFreeTcpPort,
  findFreeTcpPortInRange,
  switchToWebDriverWindow,
} from "./GateWebDriver.mjs";

test("clears an owned driver only after its teardown is confirmed", async () => {
  const failedDriver = {
    async dispose() {
      throw new Error("driver process remained alive");
    },
  };
  let retainedDriver = failedDriver;
  await assert.rejects(
    async () => {
      retainedDriver = await disposeConfirmedWebDriver(retainedDriver);
    },
    /remained alive/,
  );
  assert.equal(
    retainedDriver,
    failedDriver,
    "a rejected teardown must retain the owned process reference",
  );

  let disposeCount = 0;
  let releasedDriver = {
    async dispose() {
      disposeCount += 1;
    },
  };
  releasedDriver = await disposeConfirmedWebDriver(releasedDriver);
  assert.equal(disposeCount, 1);
  assert.equal(releasedDriver, undefined);
});

test("allocates a loopback port that the gate can bind", async () => {
  const port = await findFreeTcpPort();
  const server = http.createServer((_request, response) => response.end());

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", resolve);
  });
  await new Promise((resolve, reject) =>
    server.close((error) => (error ? reject(error) : resolve())),
  );
});

test("allocates reusable debug ports outside the Windows ephemeral range", async () => {
  const first = await findFreeTcpPortInRange(40_000, 40_100);
  const second = await findFreeTcpPortInRange(40_000, 40_100);

  assert.ok(first >= 40_000 && first <= 40_100);
  assert.ok(second >= 40_000 && second <= 40_100);
  assert.notEqual(first, second);
});

test("uses the shared bounded W3C response contract", async () => {
  const port = await findFreeTcpPort();
  const server = http.createServer((request, response) => {
    response.setHeader("content-type", "application/json");
    if (request.url === "/error") {
      response.statusCode = 400;
      response.end(
        JSON.stringify({ value: { error: "invalid argument", message: "rejected" } }),
      );
      return;
    }
    if (request.url === "/slow") {
      setTimeout(() => response.end(JSON.stringify({ value: "late" })), 200);
      return;
    }
    response.end(JSON.stringify({ value: { sessionId: "session-01" } }));
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", resolve);
  });

  try {
    const request = createWebDriverClient(`http://127.0.0.1:${port}`, {
      defaultTimeoutMilliseconds: 2_000,
    });
    assert.deepEqual(await request("GET", "/status"), {
      sessionId: "session-01",
    });
    const protocolError = await request("POST", "/error", {}).then(
      () => undefined,
      (error) => error,
    );
    assert.match(protocolError.message, /invalid argument/);
    assert.equal(protocolError.webDriverError, "invalid argument");
    await assert.rejects(request("GET", "/slow", undefined, 25), {
      name: "TimeoutError",
    });
  } finally {
    await new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});

test("serializes a parameterless W3C POST command as an empty JSON object", async () => {
  const port = await findFreeTcpPort();
  let observedRequest;
  const server = http.createServer((request, response) => {
    const chunks = [];
    request.on("data", (chunk) => chunks.push(chunk));
    request.on("end", () => {
      observedRequest = {
        body: Buffer.concat(chunks).toString("utf8"),
        contentType: request.headers["content-type"],
        method: request.method,
        url: request.url,
      };
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify({ value: null }));
    });
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", resolve);
  });

  try {
    const request = createWebDriverClient(`http://127.0.0.1:${port}`);
    await request(
      "POST",
      "/session/session-01/element/element-01/click",
    );
    assert.deepEqual(observedRequest, {
      body: "{}",
      contentType: "application/json",
      method: "POST",
      url: "/session/session-01/element/element-01/click",
    });
  } finally {
    await new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});

test("resamples an owned dialog after a stale WebDriver window snapshot", async () => {
  const handleSnapshots = [["graphics-dialog"], ["graphics-dialog"]];
  const urls = [
    "data:,",
    "tauri://localhost/project-dialog.html?kind=graphicsFailure",
  ];
  let currentHandle;
  let handleReadCount = 0;
  let urlReadCount = 0;
  const driver = {
    sessionId: "session-01",
    async request(method, endpoint, body) {
      if (method === "GET" && endpoint.endsWith("/window/handles")) {
        const snapshot =
          handleSnapshots[Math.min(handleReadCount, handleSnapshots.length - 1)];
        handleReadCount += 1;
        return snapshot;
      }
      if (method === "POST" && endpoint.endsWith("/window")) {
        currentHandle = body.handle;
        return null;
      }
      if (method === "POST" && endpoint.endsWith("/execute/sync")) {
        assert.equal(currentHandle, "graphics-dialog");
        const url = urls[Math.min(urlReadCount, urls.length - 1)];
        urlReadCount += 1;
        return url;
      }
      throw new Error(`unexpected WebDriver request: ${method} ${endpoint}`);
    },
  };

  const selected = await switchToWebDriverWindow(
    driver,
    (url) => new URL(url).pathname.endsWith("/project-dialog.html"),
    "graphics Project dialog",
    500,
  );

  assert.deepEqual(selected, {
    handle: "graphics-dialog",
    url: urls[1],
  });
  assert.equal(handleReadCount, 2);
  assert.equal(urlReadCount, 2);
});

test("resamples after a window closes between handle discovery and switch", async () => {
  const noSuchWindow = Object.assign(new Error("the handle closed"), {
    webDriverError: "no such window",
  });
  const wrappedNoSuchWindow = new Error("the WebDriver switch failed", {
    cause: noSuchWindow,
  });
  let currentHandle;
  let handleReadCount = 0;
  const driver = {
    sessionId: "session-01",
    async request(method, endpoint, body) {
      if (method === "GET" && endpoint.endsWith("/window/handles")) {
        handleReadCount += 1;
        return handleReadCount === 1 ? ["closed"] : ["target"];
      }
      if (method === "POST" && endpoint.endsWith("/window")) {
        if (body.handle === "closed") throw wrappedNoSuchWindow;
        currentHandle = body.handle;
        return null;
      }
      if (method === "POST" && endpoint.endsWith("/execute/sync")) {
        assert.equal(currentHandle, "target");
        return "tauri://localhost/project-dialog.html";
      }
      throw new Error(`unexpected WebDriver request: ${method} ${endpoint}`);
    },
  };

  const selected = await switchToWebDriverWindow(
    driver,
    (url) => new URL(url).pathname.endsWith("/project-dialog.html"),
    "graphics Project dialog",
    500,
  );

  assert.deepEqual(selected, {
    handle: "target",
    url: "tauri://localhost/project-dialog.html",
  });
  assert.equal(handleReadCount, 2);
});

test("limits every WebDriver request to the remaining discovery budget", async () => {
  const observedTimeouts = [];
  const driver = {
    sessionId: "session-01",
    async request(_method, _endpoint, _body, timeoutMilliseconds) {
      observedTimeouts.push(timeoutMilliseconds);
      await new Promise((_resolve, reject) =>
        setTimeout(
          () =>
            reject(
              Object.assign(new Error("request timed out"), {
                name: "TimeoutError",
              }),
            ),
          timeoutMilliseconds ?? 500,
        ),
      );
    },
  };
  const startedAt = performance.now();

  await assert.rejects(
    switchToWebDriverWindow(
      driver,
      () => false,
      "slow Project dialog",
      30,
    ),
    { name: "TimeoutError" },
  );

  assert.ok(performance.now() - startedAt < 200);
  assert.ok(
    observedTimeouts.every(
      (timeout) => Number.isFinite(timeout) && timeout > 0 && timeout <= 30,
    ),
  );
});

test("fails immediately on a non-transient WebDriver protocol error", async () => {
  const protocolError = Object.assign(new Error("invalid handle"), {
    webDriverError: "invalid argument",
  });
  const wrappedProtocolError = new Error("the WebDriver switch failed", {
    cause: protocolError,
  });
  let handleReadCount = 0;
  const driver = {
    sessionId: "session-01",
    async request(method, endpoint) {
      if (method === "GET" && endpoint.endsWith("/window/handles")) {
        handleReadCount += 1;
        return ["invalid"];
      }
      throw wrappedProtocolError;
    },
  };
  const startedAt = performance.now();

  await assert.rejects(
    switchToWebDriverWindow(
      driver,
      () => false,
      "invalid Project dialog",
      5_000,
    ),
    (error) => error === wrappedProtocolError,
  );

  assert.equal(handleReadCount, 1);
  assert.ok(performance.now() - startedAt < 200);
});
