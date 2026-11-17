import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
export const engineRoot = fileURLToPath(new URL('../', import.meta.url));
export function rustTool(name) {
  const local = join(process.env.CARGO_HOME || join(homedir(), '.cargo'), 'bin', process.platform === 'win32' ? `${name}.exe` : name);
  return existsSync(local) ? local : name;
}
export function run(name, args) {
  const result = spawnSync(rustTool(name), args, { cwd: engineRoot, stdio: 'inherit' });
  if (result.error) throw new Error(`Cannot run ${name}: ${result.error.message}. See README.md for setup.`);
  if (result.status !== 0) process.exit(result.status ?? 1);
}
