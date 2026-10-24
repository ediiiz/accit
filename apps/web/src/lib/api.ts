// Mirrors the Portal API (docs/spec.md section 8); kept in step with crates/server/src/portal by hand.

export type PortalState = "none" | "active" | "expired" | "trusted";

export interface PortalApp {
  name: string;
  url: string;
}

export interface PortalStatus {
  state: PortalState;
  /** Unix seconds; `null` without a Grant. */
  expiry: number | null;
  server_time: number;
  address: string;
  family: "ipv4" | "ipv6";
  apps: PortalApp[];
  title: string;
  default_language: string;
  csrf_token: string;
}

export interface Api {
  status(): Promise<PortalStatus>;
  unlock(csrfToken: string): Promise<PortalStatus>;
  revoke(csrfToken: string): Promise<PortalStatus>;
}

export class ApiError extends Error {
  readonly kind: "rate_limited" | "unavailable";
  /** Seconds to wait; set for `rate_limited`. */
  readonly retryAfter: number;

  constructor(kind: ApiError["kind"], retryAfter = 0) {
    super(kind);
    this.kind = kind;
    this.retryAfter = retryAfter;
  }
}

type Fetch = (input: string, init?: RequestInit) => Promise<Response>;

const DEFAULT_RETRY_SECONDS = 60;

function retryAfter(response: Response): number {
  const seconds = Number.parseInt(response.headers.get("retry-after") ?? "", 10);

  return seconds > 0 ? seconds : DEFAULT_RETRY_SECONDS;
}

export function createApi(fetchImpl: Fetch = (input, init) => fetch(input, init)): Api {
  async function call(path: string, init: RequestInit): Promise<PortalStatus> {
    let response: Response;

    try {
      response = await fetchImpl(path, { ...init, credentials: "same-origin" });
    } catch {
      throw new ApiError("unavailable");
    }

    if (response.status === 429) {
      throw new ApiError("rate_limited", retryAfter(response));
    }

    if (!response.ok) {
      throw new ApiError("unavailable");
    }

    try {
      // SAFETY: the Portal API answers this shape (docs/spec.md section 8); crates/server tests pin it.
      return (await response.json()) as PortalStatus;
    } catch {
      throw new ApiError("unavailable");
    }
  }

  const post = (path: string, csrfToken: string) =>
    call(path, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken },
      body: "{}",
    });

  return {
    status: () => call("/api/status", {}),
    unlock: (csrfToken) => post("/api/unlock", csrfToken),
    revoke: (csrfToken) => post("/api/revoke", csrfToken),
  };
}
