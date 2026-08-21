import assert from "node:assert/strict";
import http from "node:http";
import test from "node:test";

import {
  createWebDriverClient,
  disposeConfirmedWebDriver,
  findFreeTcpPort,
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
    await assert.rejects(request("POST", "/error", {}), /invalid argument/);
    await assert.rejects(request("GET", "/slow", undefined, 25), {
      name: "TimeoutError",
    });
  } finally {
    await new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  }
});
