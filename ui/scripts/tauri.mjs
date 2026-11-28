import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const command = fileURLToPath(new URL("../node_modules/.bin/tauri", import.meta.url));
const root = fileURLToPath(new URL("../../", import.meta.url));
const result = spawnSync(command, [...process.argv.slice(2), "--config", "src-tauri/tauri.conf.json"], {
  cwd: root,
  stdio: "inherit",
});
process.exit(result.status ?? 1);
