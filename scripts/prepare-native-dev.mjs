import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, readdirSync, rmSync } from 'node:fs';
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

  const preparedFrameworksDir = join(
    'src-tauri',
    'libs',
    'macos-bundle',
  );
  const developmentFrameworksDir = join('src-tauri', 'target', 'Frameworks');
  rmSync(developmentFrameworksDir, { recursive: true, force: true });
  mkdirSync(developmentFrameworksDir, { recursive: true });
  const nativeEntries = readdirSync(preparedFrameworksDir, {
    withFileTypes: true,
  });
  for (const entry of nativeEntries) {
    cpSync(
      join(preparedFrameworksDir, entry.name),
      join(developmentFrameworksDir, entry.name),
      {
        recursive: true,
        force: true,
        dereference: false,
        preserveTimestamps: true,
        verbatimSymlinks: true,
      },
    );
  }

  const composerResourcesSourceDir = join(
    'src-tauri',
    'libs',
    'macos',
    'share',
    'composer',
  );
  const composerResourcesDevelopmentParentDir = join(
    'src-tauri',
    'target',
    'debug',
    'share',
  );
  const composerResourcesDevelopmentDir = join(
    composerResourcesDevelopmentParentDir,
    'composer',
  );
  rmSync(composerResourcesDevelopmentDir, { recursive: true, force: true });
  mkdirSync(composerResourcesDevelopmentParentDir, {
    recursive: true,
  });
  cpSync(composerResourcesSourceDir, composerResourcesDevelopmentDir, {
    recursive: true,
    force: true,
    dereference: false,
    preserveTimestamps: true,
    verbatimSymlinks: true,
  });

  console.log(
    `Synced macOS development native files: ${nativeEntries
      .map((entry) => entry.name)
      .join(', ')}`,
  );

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
