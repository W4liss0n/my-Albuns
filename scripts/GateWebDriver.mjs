import net from "node:net";

export async function findFreeTcpPort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
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
