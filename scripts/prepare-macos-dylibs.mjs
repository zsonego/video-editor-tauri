import {
  chmodSync,
  cpSync,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import {
  basename,
  dirname,
  isAbsolute,
  join,
  relative,
  resolve,
} from 'node:path';
import { spawnSync } from 'node:child_process';

const rootDir = process.cwd();
const tauriDir = join(rootDir, 'src-tauri');
const sourceLibsDir = join(tauriDir, 'libs', 'macos');
const bundleLibsDir = join(tauriDir, 'libs', 'macos-bundle');
const entrySourceDylib = join(sourceLibsDir, 'libcomposer.dylib');
const tauriConfigPaths = [
  join(tauriDir, 'tauri.macos.conf.json'),
  join(tauriDir, 'tauri.dmg.conf.json'),
  join(tauriDir, 'tauri.appstore.conf.json'),
];
const dependencySigningIdentity =
  process.env.APPLE_DYLIB_SIGNING_IDENTITY ||
  '-';
const systemPrefixes = [
  '/usr/lib/',
  '/System/Library/',
  '/Library/Apple/System/',
];

if (process.platform !== 'darwin') {
  console.error(
    'prepare:macos:dylibs can only run on macOS because it requires otool, lipo, and install_name_tool.',
  );
  process.exit(1);
}

if (!existsSync(entrySourceDylib)) {
  console.error(`Entry dylib not found: ${entrySourceDylib}`);
  process.exit(1);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    ...options,
  });

  if (result.error) {
    throw new Error(`${command} failed: ${result.error.message}`);
  }

  if (result.status !== 0) {
    const output = [result.stdout, result.stderr]
      .filter(Boolean)
      .join('\n')
      .trim();
    throw new Error(
      `${command} ${args.join(' ')} failed${output ? `\n${output}` : ''}`,
    );
  }

  return result.stdout ?? '';
}

function parseOtoolLibraries(output) {
  return output
    .split(/\r?\n/)
    .slice(1)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => line.match(/^(.+?)\s+\(/)?.[1])
    .filter(Boolean);
}

function getLibraries(filePath) {
  return parseOtoolLibraries(run('otool', ['-L', filePath]));
}

function getRpaths(filePath) {
  const output = run('otool', ['-l', filePath]);
  const lines = output.split(/\r?\n/);
  const rpaths = [];
  let readingRpath = false;

  for (const line of lines) {
    const trimmed = line.trim();

    if (trimmed === 'cmd LC_RPATH') {
      readingRpath = true;
      continue;
    }

    if (readingRpath && trimmed.startsWith('path ')) {
      const rpath = trimmed.match(/^path\s+(.+?)\s+\(offset\s+\d+\)$/)?.[1];
      if (rpath) {
        rpaths.push(rpath);
      }
      readingRpath = false;
    }
  }

  return rpaths;
}

function isSystemLibrary(ref) {
  return systemPrefixes.some((prefix) => ref.startsWith(prefix));
}

function resolveTokenPath(value, consumerPath) {
  if (value.startsWith('@loader_path/')) {
    return resolve(dirname(consumerPath), value.slice('@loader_path/'.length));
  }

  if (value.startsWith('@executable_path/')) {
    const suffix = value.slice('@executable_path/'.length);
    if (suffix.startsWith('../Frameworks/')) {
      return resolve(sourceLibsDir, suffix.slice('../Frameworks/'.length));
    }
    return resolve(sourceLibsDir, suffix);
  }

  return value;
}

function resolveDependency(ref, consumerPath) {
  if (isSystemLibrary(ref)) {
    return null;
  }

  if (ref.startsWith('@loader_path/') || ref.startsWith('@executable_path/')) {
    const resolvedPath = resolveTokenPath(ref, consumerPath);
    return existsSync(resolvedPath) ? resolvedPath : null;
  }

  if (ref.startsWith('@rpath/')) {
    const relativePath = ref.slice('@rpath/'.length);
    const rpaths = getRpaths(consumerPath);
    const candidates = [
      ...rpaths.map((rpath) =>
        join(resolveTokenPath(rpath, consumerPath), relativePath),
      ),
      join(dirname(consumerPath), relativePath),
      join(sourceLibsDir, relativePath),
    ];

    return candidates.find((candidate) => existsSync(candidate)) ?? null;
  }

  return existsSync(ref) ? ref : null;
}

function toFrameworkPath(filePath) {
  return relative(tauriDir, filePath).replaceAll('\\', '/');
}

function ensureInsideBundleLibsDir(filePath) {
  const relativePath = relative(bundleLibsDir, filePath);
  return (
    relativePath === '' ||
    (!relativePath.startsWith('..') && !isAbsolute(relativePath))
  );
}

function findFrameworkRoot(filePath) {
  let current = resolve(filePath);
  while (true) {
    if (basename(current).endsWith('.framework')) {
      return current;
    }
    const parent = dirname(current);
    if (parent === current) return null;
    current = parent;
  }
}

function describeDependency(sourcePath) {
  const sourceRealPath = realpathSync(sourcePath);
  const frameworkRoot = findFrameworkRoot(sourceRealPath);

  if (frameworkRoot) {
    const sourceBundlePath = realpathSync(frameworkRoot);
    const binaryRelativePath = relative(sourceBundlePath, sourceRealPath);
    const destBundlePath = join(bundleLibsDir, basename(sourceBundlePath));
    return {
      kind: 'framework',
      sourcePath: sourceRealPath,
      sourceRealPath,
      sourceBundlePath,
      destBundlePath,
      destPath: join(destBundlePath, binaryRelativePath),
      destKey: basename(destBundlePath),
    };
  }

  const destPath = join(bundleLibsDir, basename(sourceRealPath));
  return {
    kind: 'dylib',
    sourcePath: sourceRealPath,
    sourceRealPath,
    sourceBundlePath: sourceRealPath,
    destBundlePath: destPath,
    destPath,
    destKey: basename(destPath),
  };
}

function loaderPathReference(consumer, dependency) {
  const relativePath = relative(dirname(consumer.destPath), dependency.destPath)
    .replaceAll('\\', '/');
  return `@loader_path/${relativePath}`;
}

function assertArm64(filePath) {
  const output = run('lipo', ['-info', filePath]);
  if (!output.includes('arm64')) {
    throw new Error(
      `${filePath} is missing arm64 architecture: ${output.trim()}`,
    );
  }
}

function updateTauriConfig(configPath, frameworkPaths) {
  const config = JSON.parse(readFileSync(configPath, 'utf8'));
  config.bundle ??= {};
  config.bundle.macOS ??= {};
  config.bundle.macOS.frameworks = frameworkPaths;
  writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);
}

function discoverDependencies() {
  const itemsByDest = new Map();
  const pending = [];
  const unresolved = [];

  function addItem(sourcePath, requestedBy = null) {
    const descriptor = describeDependency(sourcePath);
    const existing = itemsByDest.get(descriptor.destKey);

    if (existing && existing.sourceRealPath !== descriptor.sourceRealPath) {
      throw new Error(
        [
          `Found duplicate native dependency names from different sources: ${descriptor.destKey}`,
          `Existing: ${existing.sourceBundlePath}`,
          `New: ${descriptor.sourceBundlePath}`,
          requestedBy ? `Referenced by: ${requestedBy}` : '',
        ]
          .filter(Boolean)
          .join('\n'),
      );
    }

    if (existing) {
      return existing;
    }

    const item = {
      ...descriptor,
      dependencies: [],
      scanned: false,
    };

    itemsByDest.set(descriptor.destKey, item);
    pending.push(item);
    return item;
  }

  addItem(entrySourceDylib);

  while (pending.length > 0) {
    const item = pending.shift();
    if (item.scanned) {
      continue;
    }

    item.scanned = true;
    const refs = getLibraries(item.sourcePath);

    for (const ref of refs) {
      if (isSystemLibrary(ref)) {
        continue;
      }

      const resolved = resolveDependency(ref, item.sourcePath);
      if (!resolved) {
        unresolved.push({ ref, consumer: item.sourcePath });
        continue;
      }

      const resolvedRealPath = realpathSync(resolved);
      if (resolvedRealPath === item.sourceRealPath) {
        continue;
      }

      const dep = addItem(resolved, item.sourcePath);
      item.dependencies.push({
        originalRef: ref,
        replacementRef: loaderPathReference(item, dep),
      });
    }
  }

  if (unresolved.length > 0) {
    const detail = unresolved
      .map(({ ref, consumer }) => `- ${ref}\n  Referenced by: ${consumer}`)
      .join('\n');
    throw new Error(
      `Found unresolved non-system dependencies. Check local paths or rpaths first:\n${detail}`,
    );
  }

  return [...itemsByDest.values()];
}

function resetBundleLibsDir() {
  rmSync(bundleLibsDir, { recursive: true, force: true });
  mkdirSync(bundleLibsDir, { recursive: true });
}

function removeCopiedBundleMetadata(directoryPath) {
  for (const name of readdirSync(directoryPath)) {
    const childPath = join(directoryPath, name);
    if (name === '.DS_Store' || name === '_CodeSignature') {
      rmSync(childPath, { recursive: true, force: true });
      continue;
    }
    const stat = lstatSync(childPath);
    if (stat.isDirectory() && !stat.isSymbolicLink()) {
      removeCopiedBundleMetadata(childPath);
    }
  }
}

function copyNativeDependencies(items) {
  for (const item of items) {
    if (!ensureInsideBundleLibsDir(item.destBundlePath)) {
      throw new Error(
        `Destination is outside libs/macos-bundle: ${item.destBundlePath}`,
      );
    }

    if (item.kind === 'framework') {
      cpSync(item.sourceBundlePath, item.destBundlePath, {
        recursive: true,
        dereference: false,
        preserveTimestamps: true,
        verbatimSymlinks: true,
      });
      removeCopiedBundleMetadata(item.destBundlePath);
    } else {
      copyFileSync(item.sourcePath, item.destPath);
    }
    chmodSync(item.destPath, 0o755);
    console.log(
      `Copied ${item.kind}: ${item.sourceBundlePath} -> ${item.destBundlePath}`,
    );
  }
}

function rewriteInstallNames(items) {
  for (const item of items) {
    chmodSync(item.destPath, 0o755);
    const id = item.kind === 'framework'
      ? `@rpath/${basename(item.destBundlePath)}/${relative(item.destBundlePath, item.destPath).replaceAll('\\', '/')}`
      : `@rpath/${basename(item.destPath)}`;
    run('install_name_tool', ['-id', id, item.destPath]);

    for (const dep of item.dependencies) {
      if (dep.originalRef === dep.replacementRef) {
        continue;
      }
      run('install_name_tool', [
        '-change',
        dep.originalRef,
        dep.replacementRef,
        item.destPath,
      ]);
    }
  }
}

function signBundledNativeDependencies(items) {
  for (const item of items.filter((candidate) => candidate.kind === 'dylib')) {
    run('codesign', [
      '--force',
      '--sign',
      dependencySigningIdentity,
      item.destPath,
    ]);
    console.log(`Signed prepared dylib: ${item.destPath}`);
  }

  for (const item of items.filter((candidate) => candidate.kind === 'framework')) {
    run('codesign', [
      '--force',
      '--sign',
      dependencySigningIdentity,
      item.destBundlePath,
    ]);
    console.log(`Signed framework: ${item.destBundlePath}`);
  }
}

function main() {
  console.log(`Scanning entry dylib: ${entrySourceDylib}`);
  const items = discoverDependencies();
  const frameworks = items
    .map((item) => item.destBundlePath)
    .sort((a, b) => {
      if (basename(a) === 'libcomposer.dylib') return -1;
      if (basename(b) === 'libcomposer.dylib') return 1;
      return basename(a).localeCompare(basename(b));
    });

  resetBundleLibsDir();
  copyNativeDependencies(items);
  rewriteInstallNames(items);

  for (const item of items) {
    assertArm64(item.destPath);
  }
  signBundledNativeDependencies(items);

  const frameworkPaths = frameworks.map(toFrameworkPath);
  for (const configPath of tauriConfigPaths) {
    updateTauriConfig(configPath, frameworkPaths);
    console.log(`Updated Tauri config: ${configPath}`);
  }

  console.log('macOS dylib preparation complete:');
  for (const frameworkPath of frameworkPaths) {
    console.log(`- ${frameworkPath}`);
  }
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
