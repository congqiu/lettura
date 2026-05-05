// Post-build script: copy manifest and icons to dist
import { copyFileSync, mkdirSync, existsSync, writeFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');
const dist = resolve(root, 'dist');

// Create manifest.json for the extension
const manifest = {
  manifest_version: 3,
  name: 'Lettura',
  version: '1.2.0',
  description: 'Save articles to your Lettura instance',
  permissions: ['activeTab', 'contextMenus', 'storage'],
  action: {
    default_popup: 'src/popup/index.html',
    default_icon: {
      '16': 'icons/icon16.png',
      '48': 'icons/icon48.png',
      '128': 'icons/icon128.png',
    },
  },
  background: {
    service_worker: 'background.js',
  },
  icons: {
    '16': 'icons/icon16.png',
    '48': 'icons/icon48.png',
    '128': 'icons/icon128.png',
  },
};

// Write manifest
writeFileSync(resolve(dist, 'manifest.json'), JSON.stringify(manifest, null, 2));
console.log('Created manifest.json');

// Copy icons
const iconsDir = resolve(dist, 'icons');
if (!existsSync(iconsDir)) {
  mkdirSync(iconsDir, { recursive: true });
}

const iconSizes = [16, 48, 128];
for (const size of iconSizes) {
  const src = resolve(root, 'src', 'icons', `icon${size}.png`);
  const dest = resolve(iconsDir, `icon${size}.png`);
  if (existsSync(src)) {
    copyFileSync(src, dest);
    console.log(`Copied icon${size}.png`);
  } else {
    console.warn(`Warning: icon${size}.png not found`);
  }
}

console.log('Build complete!');
