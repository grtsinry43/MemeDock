import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));
function run(command, args, cwd = root) {
  const result = spawnSync(command, args, { cwd, stdio: 'inherit', shell: false });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
const web = fileURLToPath(new URL('../../apps/server-web/', import.meta.url));
run('pnpm', ['install', '--frozen-lockfile'], web);
run('pnpm', ['check'], web);
run('pnpm', ['build'], web);
run('cargo', ['build', '--locked', '--release', '-p', 'memedock-server', '--features', 'web']);
