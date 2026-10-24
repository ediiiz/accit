// Test doubles for the Portal API: plain objects passed in through props, never module mocks.
import { ApiError, type Api, type PortalStatus } from "./api.js";
import type { LastUnlocked } from "./portal-controller.svelte.js";

export const ADDRESS = "93.184.216.34";

export const status = (patch: Partial<PortalStatus> = {}): PortalStatus => ({
  state: "none",
  expiry: null,
  server_time: 1_800_000_000,
  address: ADDRESS,
  family: "ipv4",
  apps: [],
  title: "Access",
  default_language: "en",
  csrf_token: "token",
  ...patch,
});

export interface Deferred<T> {
  promise: Promise<T>;
  resolve(value: T): void;
}

export function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;

  const promise = new Promise<T>((done) => {
    resolve = done;
  });

  return { promise, resolve };
}

type Answer = PortalStatus | ApiError | Promise<PortalStatus>;

/**
 * Calls are logged as `name` or `name:token`; each answer comes from the queue for that name. The
 * last status answer repeats: a focus event may read the status at any time.
 */
export function fakeApi(answers: Partial<Record<keyof Api, Answer[]>>) {
  const calls: string[] = [];

  const next = (name: keyof Api, label: string) => {
    calls.push(label);
    const queue = answers[name];
    const answer = name === "status" && queue?.length === 1 ? queue[0] : queue?.shift();

    if (answer === undefined) {
      throw new Error(`unexpected ${label}`);
    }

    return answer instanceof ApiError ? Promise.reject(answer) : Promise.resolve(answer);
  };

  const api: Api = {
    status: () => next("status", "status"),
    unlock: (token) => next("unlock", `unlock:${token}`),
    revoke: (token) => next("revoke", `revoke:${token}`),
  };

  return { api, calls };
}

export function memoryStore(
  initial: string | null = null,
): LastUnlocked & { value: string | null } {
  const store = {
    value: initial,
    read: () => store.value,
    write: (address: string) => {
      store.value = address;
    },
  };

  return store;
}
