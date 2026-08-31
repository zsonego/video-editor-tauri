import { spawnSync } from 'node:child_process';

if (process.platform === 'darwin') {
  const result = spawnSync(
    process.execPath,
    ['scripts/prepare-macos-dylibs.mjs'],
    { stdio: 'inherit' },
  );

  if (result.error) {
    throw result.error;
  }
  process.exit(result.status ?? 1);
}
