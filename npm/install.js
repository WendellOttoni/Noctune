#!/usr/bin/env node
// Downloads the correct pre-compiled binary from GitHub Releases on install.
const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const REPO = 'WendellOttoni/Noctune';
const VERSION = require('./package.json').version;
const BIN_DIR = path.join(__dirname, 'bin');
const BIN_PATH = path.join(BIN_DIR, process.platform === 'win32' ? 'noctune.exe' : 'noctune');

function getArtifactName() {
  const { platform, arch } = process;
  if (platform === 'win32' && arch === 'x64') return 'noctune-windows-x64.exe';
  if (platform === 'linux' && arch === 'x64')  return 'noctune-linux-x64';
  if (platform === 'darwin' && arch === 'x64') return 'noctune-macos-x64';
  if (platform === 'darwin' && arch === 'arm64') return 'noctune-macos-arm64';
  throw new Error(`Unsupported platform: ${platform}/${arch}`);
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const follow = (u) => {
      https.get(u, { headers: { 'User-Agent': 'noctune-install' } }, (res) => {
        if (res.statusCode === 301 || res.statusCode === 302) {
          follow(res.headers.location);
          return;
        }
        if (res.statusCode !== 200) {
          reject(new Error(`HTTP ${res.statusCode} downloading ${u}`));
          return;
        }
        const file = fs.createWriteStream(dest);
        res.pipe(file);
        file.on('finish', () => file.close(resolve));
        file.on('error', reject);
      }).on('error', reject);
    };
    follow(url);
  });
}

async function main() {
  const artifact = getArtifactName();
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${artifact}`;

  fs.mkdirSync(BIN_DIR, { recursive: true });

  console.log(`noctune: downloading ${artifact}...`);
  await download(url, BIN_PATH);

  if (process.platform !== 'win32') {
    fs.chmodSync(BIN_PATH, 0o755);
  }

  console.log('noctune: installed successfully. Run: noctune');
}

main().catch((err) => {
  console.error('noctune: install failed:', err.message);
  process.exit(1);
});
