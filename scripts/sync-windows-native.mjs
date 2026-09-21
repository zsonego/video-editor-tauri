import { createHash } from 'node:crypto';
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
} from 'node:fs';
import { join, relative, resolve } from 'node:path';

const profile = process.argv[2] ?? 'debug';
const supportedProfiles = new Set(['debug', 'release']);

if (!supportedProfiles.has(profile)) {
  throw new Error(
    `Unsupported Windows native target profile: ${profile}. Expected debug or release.`,
  );
}

if (process.platform !== 'win32') {
  console.log(`Skipping Windows native sync on ${process.platform}.`);
  process.exit(0);
}

const sourceDir = resolve('src-tauri', 'libs', 'windows');
const destinationDir = resolve('src-tauri', 'target', profile);
const composerDllName = 'libcomposer.dll';

if (!existsSync(join(sourceDir, composerDllName))) {
  throw new Error(`Windows Composer library not found: ${sourceDir}`);
}

function syncDirectory(source, destination) {
  mkdirSync(destination, { recursive: true });

  for (const entry of readdirSync(source, { withFileTypes: true })) {
    const sourcePath = join(source, entry.name);
    const destinationPath = join(destination, entry.name);

    if (entry.isDirectory()) {
      syncDirectory(sourcePath, destinationPath);
      continue;
    }

    if (
      existsSync(destinationPath) &&
      sha256(sourcePath) === sha256(destinationPath)
    ) {
      continue;
    }

    cpSync(sourcePath, destinationPath, {
      force: true,
      preserveTimestamps: true,
    });
  }
}

function sha256(filePath) {
  return createHash('sha256').update(readFileSync(filePath)).digest('hex');
}

try {
  syncDirectory(sourceDir, destinationDir);
} catch (error) {
  if (error?.code === 'EBUSY' || error?.code === 'EPERM') {
    throw new Error(
      `Unable to update Windows native libraries in ${destinationDir}. ` +
        'Close every running aicut process and try again.',
      { cause: error },
    );
  }
  throw error;
}

const sourceComposerDll = join(sourceDir, composerDllName);
const destinationComposerDll = join(destinationDir, composerDllName);
const sourceHash = sha256(sourceComposerDll);
const destinationHash = sha256(destinationComposerDll);

if (sourceHash !== destinationHash) {
  throw new Error(
    `Windows Composer library verification failed: ${destinationComposerDll}`,
  );
}

console.log(
  `Synced Windows native files from ${relative(process.cwd(), sourceDir)} ` +
    `to ${relative(process.cwd(), destinationDir)} ` +
    `(libcomposer.dll sha256: ${sourceHash}).`,
);
