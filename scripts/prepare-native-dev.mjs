import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

if (process.platform === 'darwin') {
  const result = spawnSync(
    process.execPath,
    ['scripts/prepare-macos-dylibs.mjs'],
    { stdio: 'inherit' },
  );

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }

  const localizationSourceDir = join('src-tauri', 'infoplist');
  const debugOutputDir = join('src-tauri', 'target', 'debug');
  const localizationDirs = readdirSync(localizationSourceDir, {
    withFileTypes: true,
  }).filter((entry) => entry.isDirectory() && entry.name.endsWith('.lproj'));

  mkdirSync(debugOutputDir, { recursive: true });
  for (const entry of localizationDirs) {
    cpSync(
      join(localizationSourceDir, entry.name),
      join(debugOutputDir, entry.name),
      { recursive: true, force: true },
    );
  }

  console.log(
    `Prepared macOS development localizations: ${localizationDirs
      .map((entry) => entry.name)
      .join(', ')}`,
  );
}
