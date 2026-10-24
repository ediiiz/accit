import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { randomBytes } from "node:crypto";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { BUDGET_BYTES, checkBudget, measureInitialLoad } from "./web-budget";

const KIB = 1024;

let build: string;

function write(path: string, content: string | Uint8Array) {
  const file = join(build, path);
  mkdirSync(join(file, ".."), { recursive: true });
  writeFileSync(file, content);
}

/** Incompressible, so the gzip size is within a few bytes of the raw size. */
function noise(kib: number): Uint8Array {
  return randomBytes(kib * KIB);
}

const page = (body: string) => `<!doctype html><html><head>${body}</head><body></body></html>`;

beforeEach(() => {
  build = mkdtempSync(join(tmpdir(), "web-budget-"));
});

afterEach(() => {
  rmSync(build, { recursive: true, force: true });
});

describe("measureInitialLoad", () => {
  test("counts the page, its stylesheet, its preloads and their static imports; not lazy chunks or images", () => {
    write(
      "index.html",
      page(`
        <link rel="icon" href="./favicon.png">
        <link href="./_app/start.js" rel="modulepreload">
        <link href="./_app/app.css" rel="stylesheet">
        <script>import("./_app/start.js").then(() => import("./_app/app.js"))</script>`),
    );
    write("favicon.png", noise(50));
    write("_app/start.js", `import{a}from"./shared.js";export{a};`);
    write("_app/shared.js", `export const a=1;`);
    write("_app/app.js", `export default 1;`);
    write("_app/app.css", `body{margin:0}`);
    write("_app/lazy.js", noise(200));
    write("_app/app.js.gz", noise(200));

    const load = measureInitialLoad(build);

    expect(load.files.map((file) => file.path).sort()).toEqual([
      "_app/app.css",
      "_app/app.js",
      "_app/shared.js",
      "_app/start.js",
      "index.html",
    ]);
    expect(load.html + load.css + load.js).toBe(load.total);
  });

  test("a dynamic import inside a chunk is lazy", () => {
    write("index.html", page(`<link href="./_app/start.js" rel="modulepreload">`));
    write("_app/start.js", `export const load=()=>import("./lazy.js");`);
    write("_app/lazy.js", noise(200));

    const load = measureInitialLoad(build);

    expect(load.files.map((file) => file.path).sort()).toEqual(["_app/start.js", "index.html"]);
  });

  test("sizes are gzip, grouped by kind", () => {
    write(
      "index.html",
      page(`<link href="./a.js" rel="modulepreload"><link href="./a.css" rel="stylesheet">`),
    );
    write("a.js", noise(20));
    write("a.css", noise(10));

    const load = measureInitialLoad(build);

    expect(load.js).toBeGreaterThan(20 * KIB);
    expect(load.js).toBeLessThan(21 * KIB);
    expect(load.css).toBeGreaterThan(10 * KIB);
    expect(load.css).toBeLessThan(11 * KIB);
    expect(load.html).toBeLessThan(KIB);
  });
});

describe("checkBudget", () => {
  test("the budget is 150 KiB (docs/spec.md section 9)", () => {
    expect(BUDGET_BYTES).toBe(150 * KIB);
  });

  test("passes under 150 KiB", () => {
    write("index.html", page(`<link href="./a.js" rel="modulepreload">`));
    write("a.js", noise(140));

    expect(checkBudget(measureInitialLoad(build)).ok).toBe(true);
  });

  test("fails over 150 KiB", () => {
    write("index.html", page(`<link href="./a.js" rel="modulepreload">`));
    write("a.js", noise(151));

    expect(checkBudget(measureInitialLoad(build)).ok).toBe(false);
  });
});
