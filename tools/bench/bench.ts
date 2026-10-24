// Acceptance measurement (docs/spec.md section 14): idle RSS with 1,000 Grants and p95 /verify
// latency at 500 checks per second. Runs the release binary in the production profile behind no
// proxy, playing Traefik itself, pinned to two CPUs when `taskset` exists.
//
//   scripts/bench [path/to/accit]        env: BENCH_SECONDS (30), BENCH_CPUS ("0,1")
import { type ChildProcess, spawn, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { Agent, request } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { percentile } from "./stats";

const GRANTS = 1000;

const RATE = 500;

const WARMUP = 2;

const MAX_RSS_MIB = 64;

const MAX_P95_MS = 5;

const PORTAL = 18080;

const VERIFY = 18081;

const HOST = "portal.bench.test";

const GATED = "gated.bench.test";

const TOKEN = "bench-proxy-token";

const seconds = Number(process.env.BENCH_SECONDS ?? 30);

// 93.184.0.0/16 is public; 1,000 Addresses fit in its first four /24s.
const address = (index: number) => `93.184.${Math.floor(index / 250)}.${(index % 250) + 1}`;

type Reply = {
  status: number;
  headers: Record<string, string | string[] | undefined>;
  body: string;
};

const agent = new Agent({ keepAlive: true, maxSockets: 64 });

function send(
  port: number,
  path: string,
  method: string,
  headers: Record<string, string>,
  body = "",
): Promise<Reply> {
  return new Promise((resolve, reject) => {
    const req = request({ host: "127.0.0.1", port, path, method, headers, agent }, (res) => {
      const chunks: Buffer[] = [];
      res.on("data", (chunk: Buffer) => chunks.push(chunk));
      res.on("end", () =>
        resolve({
          status: res.statusCode ?? 0,
          headers: res.headers,
          body: Buffer.concat(chunks).toString(),
        }),
      );
    });

    req.on("error", reject);
    req.end(body);
  });
}

const proxied = (client: string, host: string) => ({
  host,
  "x-accit-proxy-token": TOKEN,
  "x-forwarded-for": client,
});

function writeConfig(dir: string): string {
  writeFileSync(join(dir, "proxy-token"), TOKEN);
  const config = join(dir, "accit.yaml");
  writeFileSync(
    config,
    `profile: production
public_origin: https://${HOST}
trusted_proxy: 127.0.0.1
proxy_token_file: ${join(dir, "proxy-token")}
database: ${join(dir, "accit.db")}
admin_socket: ${join(dir, "admin.sock")}
max_grants: ${GRANTS}
listen:
  portal: 127.0.0.1:${PORTAL}
  verify: 127.0.0.1:${VERIFY}
protection:
  mode: selected
  hosts:
    - ${GATED}
`,
  );

  return config;
}

function start(binary: string, config: string): ChildProcess {
  const env = { ...process.env, ACCIT_CONFIG: config };
  const cpus = process.env.BENCH_CPUS ?? "0,1";
  const pinned = spawnSync("taskset", ["--version"]).status === 0 && cpus !== "";
  const argv = pinned ? ["taskset", "-c", cpus, binary, "serve"] : [binary, "serve"];
  const [command = binary, ...args] = argv;

  return spawn(command, args, { env, stdio: ["ignore", "inherit", "inherit"] });
}

async function ready(): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt++) {
    const reply = await send(VERIFY, "/readyz", "GET", {
      host: "localhost",
      "x-accit-proxy-token": TOKEN,
    }).catch(() => undefined);

    if (reply?.status === 200) return;
    await new Promise((done) => setTimeout(done, 100));
  }

  throw new Error("accit did not become ready");
}

async function seed(): Promise<void> {
  const status = await send(PORTAL, "/api/status", "GET", proxied(address(0), HOST));
  const cookie = String(status.headers["set-cookie"]).split(";")[0] ?? "";
  const csrf: string = JSON.parse(status.body).csrf_token;

  for (let index = 0; index < GRANTS; index++) {
    const reply = await send(
      PORTAL,
      "/api/unlock",
      "POST",
      {
        ...proxied(address(index), HOST),
        origin: `https://${HOST}`,
        cookie,
        "x-csrf-token": csrf,
        "content-type": "application/json",
      },
      "{}",
    );

    if (reply.status !== 200) throw new Error(`unlock ${index}: ${reply.status} ${reply.body}`);
  }
}

function kibOf(pid: number, field: "VmRSS" | "VmHWM"): number {
  const line = readFileSync(`/proc/${pid}/status`, "utf8")
    .split("\n")
    .find((row) => row.startsWith(`${field}:`));

  return Number(line?.split(/\s+/)[1]);
}

/** Open loop: requests start on schedule whatever the latency, so a slow reply cannot hide behind a lower rate. */
async function load(): Promise<number[]> {
  const latencies: number[] = [];
  const pending: Promise<void>[] = [];
  const total = RATE * (seconds + WARMUP);
  const began = performance.now();

  for (let index = 0; index < total; index++) {
    const due = began + (index * 1000) / RATE;
    const wait = due - performance.now();

    if (wait > 1) await new Promise((done) => setTimeout(done, wait));
    const sentAt = performance.now();
    pending.push(
      send(VERIFY, "/verify", "GET", {
        ...proxied(address((index * 7) % GRANTS), "localhost"),
        "x-forwarded-host": GATED,
      }).then((reply) => {
        if (reply.status !== 204) throw new Error(`verify answered ${reply.status}`);

        // The load generator's own JIT warm-up would otherwise count as server latency.
        if (index >= RATE * WARMUP) latencies.push(performance.now() - sentAt);
      }),
    );
  }

  await Promise.all(pending);

  return latencies;
}

const mib = (kib: number) => (kib / 1024).toFixed(1);

type Measured = { idle: number; peak: number; latencies: number[] };

async function measure(child: ChildProcess): Promise<Measured> {
  await ready();
  await seed();
  await new Promise((done) => setTimeout(done, 5000));
  const pid = child.pid;

  if (pid === undefined) throw new Error("accit did not start");

  const idle = kibOf(pid, "VmRSS");
  const latencies = await load();

  return { idle, peak: kibOf(pid, "VmHWM"), latencies };
}

/** True when both targets hold. */
function report({ idle, peak, latencies }: Measured): boolean {
  const [p50, p95, p99] = [0.5, 0.95, 0.99].map((fraction) => percentile(latencies, fraction));

  console.log(`grants            ${GRANTS}`);
  console.log(`idle RSS          ${mib(idle)} MiB (target <= ${MAX_RSS_MIB})`);
  console.log(`peak RSS          ${mib(peak)} MiB`);
  console.log(`verify ${RATE}/s x ${seconds}s  ${latencies.length} checks`);
  console.log(
    `latency p50/p95/p99  ${p50?.toFixed(2)} / ${p95?.toFixed(2)} / ${p99?.toFixed(2)} ms (p95 target < ${MAX_P95_MS})`,
  );

  return idle / 1024 <= MAX_RSS_MIB && (p95 ?? Infinity) < MAX_P95_MS;
}

async function main(): Promise<number> {
  const binary = process.argv[2] ?? "target/release/accit";
  const dir = mkdtempSync(join(tmpdir(), "accit-bench-"));
  const child = start(binary, writeConfig(dir));

  try {
    return report(await measure(child)) ? 0 : 1;
  } finally {
    child.kill("SIGTERM");
    agent.destroy();
    rmSync(dir, { recursive: true, force: true });
  }
}

process.exit(await main());
