// Usage: bun tools/budget/web-budget-cli.ts [build-dir]   (default apps/web/build)
import { checkBudget, measureInitialLoad } from "./web-budget";

const load = measureInitialLoad(process.argv[2] ?? "apps/web/build");

const { ok, message } = checkBudget(load);

for (const file of load.files.toSorted((a, b) => b.bytes - a.bytes)) {
  console.log(`${(file.bytes / 1024).toFixed(1).padStart(7)} KiB  ${file.path}`);
}

console.log(message);

process.exit(ok ? 0 : 1);
