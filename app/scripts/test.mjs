// Runs Vitest from its canonical on-disk path.
//
// On Windows a shell with a lowercase drive letter (`e:\...`) puts `e:\...\node_modules\.bin`
// on PATH, so the Vitest CLI is loaded as `e:/...` while test files import it through Vite as
// `E:/...`. Vitest then exists twice and fails with "failed to find the runner".
import { spawnSync } from 'node:child_process';
import { realpathSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const appDir = realpathSync.native(join(dirname(fileURLToPath(import.meta.url)), '..'));
const vitest = realpathSync.native(join(appDir, 'node_modules', 'vitest', 'vitest.mjs'));
const args = ['run', '--passWithNoTests', ...process.argv.slice(2)];
const result = spawnSync(process.execPath, [vitest, ...args], { cwd: appDir, stdio: 'inherit' });
process.exit(result.status ?? 1);
