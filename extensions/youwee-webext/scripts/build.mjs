import { cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const extensionRoot = path.resolve(__dirname, '..');
const repoRoot = path.resolve(extensionRoot, '..', '..');
const srcDir = path.join(extensionRoot, 'src');
const distDir = path.join(extensionRoot, 'dist');
const chromiumInstallGuide = `YOUWEE CHROMIUM EXTENSION / TIỆN ÍCH CHROMIUM YOUWEE

ENGLISH
1. Open chrome://extensions in Chrome, Edge, Brave, Opera, Vivaldi, Arc, or Coc Coc.
2. Enable Developer mode.
3. Choose Load unpacked.
4. Select this Youwee-Extension-Chromium folder.
5. Pin Youwee Download Companion for quick access.

Keep this folder in place. Youwee updates its contents when the desktop app is updated.
If the browser still shows an older version, return to chrome://extensions and click Reload.

TIẾNG VIỆT
1. Mở chrome://extensions trong Chrome, Edge, Brave, Opera, Vivaldi, Arc hoặc Cốc Cốc.
2. Bật Chế độ dành cho nhà phát triển.
3. Chọn Tải tiện ích đã giải nén.
4. Chọn thư mục Youwee-Extension-Chromium này.
5. Ghim Youwee Download Companion để truy cập nhanh.

Giữ nguyên vị trí thư mục này. Youwee sẽ cập nhật nội dung khi ứng dụng desktop được cập nhật.
Nếu trình duyệt vẫn hiển thị bản cũ, quay lại chrome://extensions và nhấn Tải lại.
`;

async function buildTarget(target, appVersion, extensionVersion, manifestTarget = target) {
  const outDir = path.join(distDir, target);
  const manifestPath = path.join(extensionRoot, `manifest.${manifestTarget}.json`);

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
  if (target === 'chromium') {
    await writeFile(path.join(outDir, 'INSTALL.txt'), chromiumInstallGuide, 'utf8');
  }
}

async function run() {
  const packageJsonPath = path.join(repoRoot, 'package.json');
  const packageJson = JSON.parse(await readFile(packageJsonPath, 'utf8'));
  const appVersion = typeof packageJson.version === 'string' ? packageJson.version : '0.0.0';
  const manifests = await Promise.all(
    ['chromium', 'firefox'].map(async (target) =>
      JSON.parse(await readFile(path.join(extensionRoot, `manifest.${target}.json`), 'utf8')),
    ),
  );
  const extensionVersion = manifests[0].version;
  if (!/^\d+(?:\.\d+){2,3}$/.test(extensionVersion) || manifests[1].version !== extensionVersion) {
    throw new Error('Chromium and Firefox manifests must share a valid extension version.');
  }

  await rm(distDir, { recursive: true, force: true });
  await mkdir(distDir, { recursive: true });

  await buildTarget('chromium', appVersion, extensionVersion);
  await buildTarget('firefox', appVersion, extensionVersion);
  await buildTarget('firefox-amo', appVersion, extensionVersion, 'firefox');

  console.log(`Built extension packages (${appVersion} -> ${extensionVersion}):`);
  console.log(`- ${path.join(distDir, 'chromium')}`);
  console.log(`- ${path.join(distDir, 'firefox')}`);
  console.log(`- ${path.join(distDir, 'firefox-amo')}`);
}

run().catch((error) => {
  console.error(error);
  process.exit(1);
});
