// Portal initial-load budget (docs/spec.md section 9): the prerendered page, its stylesheets and
// the scripts it loads eagerly, gzip-compressed. Lazy chunks (`import()` inside a chunk) and
// images are not part of the initial load.
import { readFileSync } from "node:fs";
import { join, posix } from "node:path";
import { gzipSync } from "node:zlib";

export const BUDGET_BYTES = 150 * 1024;

export type Kind = "html" | "css" | "js";

export type Measured = { path: string; kind: Kind; bytes: number };

export type InitialLoad = Record<Kind, number> & { total: number; files: Measured[] };

const PAGE_REFS = /(?:href|src)="([^"]+\.(?:js|css))"|\bimport\(\s*"([^"]+\.js)"\s*\)/g;

const STATIC_IMPORT = /\b(?:import|export)\s*(?:[^"'()]*?\bfrom\s*)?["']([^"']+\.js)["']/g;

function references(source: string, pattern: RegExp): string[] {
  return [...source.matchAll(pattern)].map((match) => match[1] ?? match[2] ?? "");
}

function resolve(from: string, reference: string): string {
  return posix.normalize(posix.join(posix.dirname(from), reference));
}

function kindOf(path: string): Kind {
  return path.endsWith(".css") ? "css" : "js";
}

export function measureInitialLoad(buildDir: string): InitialLoad {
  const read = (path: string) => readFileSync(join(buildDir, path));
  const seen = new Map<string, Kind>([["index.html", "html"]]);

  const queue = references(read("index.html").toString("utf8"), PAGE_REFS).map((ref) =>
    resolve("index.html", ref),
  );

  for (let path = queue.pop(); path !== undefined; path = queue.pop()) {
    if (seen.has(path)) continue;
    seen.set(path, kindOf(path));

    if (kindOf(path) === "js") {
      const imports = references(read(path).toString("utf8"), STATIC_IMPORT);
      queue.push(...imports.map((ref) => resolve(path, ref)));
    }
  }

  const files = [...seen].map(([path, kind]) => ({
    path,
    kind,
    bytes: gzipSync(read(path), { level: 9 }).length,
  }));

  const sum = (kind: Kind) =>
    files.filter((file) => file.kind === kind).reduce((n, file) => n + file.bytes, 0);

  const [html, css, js] = [sum("html"), sum("css"), sum("js")];

  return { html, css, js, total: html + css + js, files };
}

export type BudgetResult = { ok: boolean; message: string };

export function checkBudget(load: InitialLoad): BudgetResult {
  const kib = (bytes: number) => (bytes / 1024).toFixed(1);
  const ok = load.total <= BUDGET_BYTES;
  const parts = `html ${kib(load.html)} + css ${kib(load.css)} + js ${kib(load.js)}`;

  return {
    ok,
    message: `${ok ? "ok" : "over budget"}: initial load ${kib(load.total)} KiB gzip of ${kib(BUDGET_BYTES)} KiB (${parts})`,
  };
}
