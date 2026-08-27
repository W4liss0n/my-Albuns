import net from "node:net";

const reservedTcpPorts = new Set();

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
    const response = await fetch(`${baseUrl}${endpoint}`, {
      method,
      headers:
        body === undefined ? undefined : { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
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
