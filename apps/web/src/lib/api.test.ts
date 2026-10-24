import { describe, expect, it } from "vitest";
import { ApiError, createApi, type PortalStatus } from "./api.js";

const status: PortalStatus = {
  state: "none",
  expiry: null,
  server_time: 1_800_000_000,
  address: "93.184.216.34",
  family: "ipv4",
  apps: [],
  title: "Access",
  default_language: "en",
  csrf_token: "token-1",
};

type Call = { url: string; init: RequestInit | undefined };

function fakeFetch(reply: () => Response) {
  const calls: Call[] = [];

  const fetch = (url: string, init?: RequestInit) => {
    calls.push({ url, init });

    return Promise.resolve(reply());
  };

  return { calls, fetch };
}

const json = (body: PortalStatus) => Response.json(body);

describe("status", () => {
  it("reads /api/status with the session cookie", async () => {
    const { calls, fetch } = fakeFetch(() => json(status));

    expect(await createApi(fetch).status()).toEqual(status);
    expect(calls).toHaveLength(1);
    expect(calls[0]?.url).toBe("/api/status");
    expect(calls[0]?.init?.credentials).toBe("same-origin");
    expect(calls[0]?.init?.method ?? "GET").toBe("GET");
  });
});

describe("mutations", () => {
  it.each([
    ["unlock", "/api/unlock"],
    ["revoke", "/api/revoke"],
  ] as const)("%s posts an empty JSON object with the CSRF token", async (action, path) => {
    const { calls, fetch } = fakeFetch(() => json({ ...status, state: "active" }));

    const result = await createApi(fetch)[action]("token-1");
    const init = calls[0]?.init;

    expect(result.state).toBe("active");
    expect(calls[0]?.url).toBe(path);
    expect(init?.method).toBe("POST");
    expect(init?.credentials).toBe("same-origin");
    expect(init?.body).toBe("{}");
    expect(new Headers(init?.headers).get("content-type")).toBe("application/json");
    expect(new Headers(init?.headers).get("x-csrf-token")).toBe("token-1");
  });
});

describe("failures", () => {
  it("429 carries Retry-After in seconds", async () => {
    const { fetch } = fakeFetch(
      () => new Response("Too Many Requests", { status: 429, headers: { "Retry-After": "42" } }),
    );

    const error = await createApi(fetch)
      .unlock("t")
      .catch((caught: Error) => caught);

    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({ kind: "rate_limited", retryAfter: 42 });
  });

  it("429 without a usable Retry-After waits a minute", async () => {
    const { fetch } = fakeFetch(
      () => new Response("", { status: 429, headers: { "Retry-After": "soon" } }),
    );

    await expect(createApi(fetch).unlock("t")).rejects.toMatchObject({
      kind: "rate_limited",
      retryAfter: 60,
    });
  });

  it.each([403, 409, 503])("%i is unavailable", async (code) => {
    const { fetch } = fakeFetch(() => new Response("no", { status: code }));

    await expect(createApi(fetch).status()).rejects.toMatchObject({ kind: "unavailable" });
  });

  it("a network error is unavailable", async () => {
    const fetch = () => Promise.reject(new TypeError("offline"));

    await expect(createApi(fetch).status()).rejects.toMatchObject({ kind: "unavailable" });
  });
});
