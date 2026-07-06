import { existsSync, mkdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { spawnSync } from 'node:child_process';

const rootDir = process.cwd();
const tauriConfig = JSON.parse(
  readFileSync(join(rootDir, 'src-tauri', 'tauri.conf.json'), 'utf8'),
);

const target = process.env.TAURI_MAS_TARGET || 'aarch64-apple-darwin';
const productName = process.env.TAURI_MAS_PRODUCT_NAME || tauriConfig.productName;
const appPath =
  process.env.TAURI_MAS_APP_PATH ||
  join(
    rootDir,
    'src-tauri',
    'target',
    target,
    'release',
    'bundle',
    'macos',
    `${productName}.app`,
  );
const pkgPath =
  process.env.TAURI_MAS_PKG_PATH ||
  join(
    rootDir,
    'src-tauri',
    'target',
    target,
    'release',
    'bundle',
    'macos',
    `${productName}-mas.pkg`,
  );
const installerIdentity = process.env.APPLE_INSTALLER_SIGNING_IDENTITY;

if (!installerIdentity) {
  console.error(
    'Missing APPLE_INSTALLER_SIGNING_IDENTITY. Use your Mac Installer Distribution certificate name.',
  );
  console.error(
    'Example: APPLE_INSTALLER_SIGNING_IDENTITY="3rd Party Mac Developer Installer: Loogear Co., Ltd. (PMBZKYYRRP)" npm run build:mas:pkg',
  );
  process.exit(1);
}

if (!existsSync(appPath)) {
  console.error(`App bundle not found: ${appPath}`);
  console.error('Run npm run build:mas:app first.');
  process.exit(1);
}

mkdirSync(dirname(pkgPath), { recursive: true });

const result = spawnSync(
  'xcrun',
  [
    'productbuild',
    '--sign',
    installerIdentity,
    '--component',
    appPath,
    '/Applications',
    pkgPath,
  ],
  { stdio: 'inherit' },
);

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

console.log(`Created Mac App Store pkg: ${pkgPath}`);
