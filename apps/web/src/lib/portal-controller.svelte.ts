import { ApiError, type Api, type PortalStatus } from "./api.js";
import { screenFor, type Screen } from "./portal.js";

export interface LastUnlocked {
  read(): string | null;
  write(address: string): void;
}

export type Mutation = "unlock" | "revoke";

export type Action = "status" | Mutation;

interface Failure {
  action: Action;
  rateLimited: boolean;
}

const KEY = "accit.last-unlocked";

/** The Address of the last successful Unlock; storage can be blocked, then nothing is remembered. */
export const browserLastUnlocked: LastUnlocked = {
  read() {
    try {
      return localStorage.getItem(KEY);
    } catch {
      return null;
    }
  },
  write(address) {
    try {
      localStorage.setItem(KEY, address);
    } catch {
      // Not remembered: a changed connection then shows as plain "not enabled".
    }
  },
};

/**
 * The Portal's state machine. Loading and focus only read; a mutation reads the status again first,
 * so it carries a live CSRF token and acts on the current state.
 */
export class PortalController {
  status = $state.raw<PortalStatus | null>(null);
  secondsLeft = $state(0);
  #working = $state<Mutation | null>(null);
  #failure = $state.raw<Failure | null>(null);
  #lastUnlocked: string | null;
  /** The newest read or mutation; an older read that answers later is dropped. */
  #latest = 0;
  #timer: ReturnType<typeof setInterval> | undefined;
  readonly #api: Api;
  readonly #store: LastUnlocked;

  constructor(api: Api, store: LastUnlocked) {
    this.#api = api;
    this.#store = store;
    this.#lastUnlocked = store.read();
  }

  screen: Screen = $derived.by(() => {
    if (this.#working === "unlock") {
      return "submitting";
    }

    if (this.#failure) {
      return this.#failure.rateLimited ? "rate_limited" : "unavailable";
    }

    return this.status ? screenFor(this.status, this.#lastUnlocked) : "loading";
  });

  get revoking(): boolean {
    return this.#working === "revoke";
  }

  get failedAction(): Action | null {
    return this.#failure?.action ?? null;
  }

  async load(): Promise<void> {
    if (this.#working) {
      return;
    }

    const ticket = ++this.#latest;

    try {
      const status = await this.#api.status();

      if (ticket === this.#latest) {
        this.#loaded(status);
      }
    } catch (error) {
      if (ticket === this.#latest) {
        this.#loadFailed(error instanceof ApiError ? error : new ApiError("unavailable"));
      }
    }
  }

  unlock(): Promise<void> {
    return this.#mutate("unlock");
  }

  revoke(): Promise<void> {
    return this.#mutate("revoke");
  }

  retry(): Promise<void> {
    const failure = this.#failure;

    if (!failure) {
      return Promise.resolve();
    }

    if (failure.action === "status") {
      return this.load();
    }

    // A failed Revoke is confirmed again, through `revoke()`.
    return this.secondsLeft > 0 || failure.action === "revoke"
      ? Promise.resolve()
      : this.#mutate(failure.action);
  }

  destroy(): void {
    clearInterval(this.#timer);
  }

  async #mutate(action: Mutation): Promise<void> {
    if (this.#working) {
      return;
    }

    this.#working = action;
    this.#failure = null;
    this.#latest++;

    try {
      const fresh = await this.#api.status();

      this.status = fresh;

      if (action === "unlock" ? fresh.state !== "trusted" : fresh.state === "active") {
        this.status = await this.#api[action](fresh.csrf_token);
        this.#remember(action, this.status);
      }
    } catch (error) {
      this.#fail(action, error instanceof ApiError ? error : new ApiError("unavailable"));
    } finally {
      this.#working = null;
    }
  }

  #loaded(status: PortalStatus): void {
    this.status = status;

    if (this.#failure?.action === "status") {
      this.#failure = null;
    }
  }

  /** A failed refresh leaves a failed mutation's screen alone. */
  #loadFailed(error: ApiError): void {
    if (!this.#failure || this.#failure.action === "status") {
      this.#fail("status", error);
    }
  }

  #remember(action: Mutation, result: PortalStatus): void {
    if (action === "unlock" && result.state === "active") {
      this.#store.write(result.address);
      this.#lastUnlocked = result.address;
    }
  }

  #fail(action: Action, error: ApiError): void {
    const wait = error.kind === "rate_limited" ? error.retryAfter : 0;

    this.#failure = { action, rateLimited: wait > 0 };
    this.#countDown(wait);
  }

  #countDown(seconds: number): void {
    clearInterval(this.#timer);
    this.secondsLeft = seconds;

    if (seconds > 0) {
      this.#timer = setInterval(() => {
        this.secondsLeft -= 1;

        if (this.secondsLeft <= 0) {
          clearInterval(this.#timer);
        }
      }, 1000);
    }
  }
}
