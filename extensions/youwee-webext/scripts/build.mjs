import { cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const extensionRoot = path.resolve(__dirname, '..');
const repoRoot = path.resolve(extensionRoot, '..', '..');
const srcDir = path.join(extensionRoot, 'src');
const distDir = path.join(extensionRoot, 'dist');

function extensionVersionFromAppVersion(appVersion) {
  const match = appVersion.match(/^(\d+)\.(\d+)\.(\d+)(?:-custom\.(\d+))?$/);
  if (!match) {
    throw new Error(`Cannot derive a browser extension version from '${appVersion}'.`);
  }

  return match.slice(1).filter(Boolean).join('.');
}

async function buildTarget(target, appVersion, extensionVersion) {
  const outDir = path.join(distDir, target);
  const manifestPath = path.join(extensionRoot, `manifest.${target}.json`);

  await mkdir(outDir, { recursive: true });
  await cp(srcDir, outDir, { recursive: true });

  const manifestContent = await readFile(manifestPath, 'utf8');
  const manifest = JSON.parse(manifestContent);
  manifest.version = extensionVersion;
  manifest.version_name = appVersion;
  await writeFile(
    path.join(outDir, 'manifest.json'),
    `${JSON.stringify(manifest, null, 2)}\n`,
    'utf8',
  );
}

async function run() {
  const packageJsonPath = path.join(repoRoot, 'package.json');
  const packageJson = JSON.parse(await readFile(packageJsonPath, 'utf8'));
  const appVersion = typeof packageJson.version === 'string' ? packageJson.version : '0.0.0';
  const extensionVersion = extensionVersionFromAppVersion(appVersion);

  await rm(distDir, { recursive: true, force: true });
  await mkdir(distDir, { recursive: true });

  await buildTarget('chromium', appVersion, extensionVersion);
  await buildTarget('firefox', appVersion, extensionVersion);

  console.log(`Built extension packages (${appVersion} -> ${extensionVersion}):`);
  console.log(`- ${path.join(distDir, 'chromium')}`);
  console.log(`- ${path.join(distDir, 'firefox')}`);
}

run().catch((error) => {
  console.error(error);
  process.exit(1);
});
