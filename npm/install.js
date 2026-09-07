#!/usr/bin/env node
// Downloads the correct pre-compiled binary from GitHub Releases on install.
const https = require('https');
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

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
    const follow = (u, redirects = 0) => {
      if (redirects > 5 || new URL(u).protocol !== 'https:') return reject(new Error('Invalid download redirect'));
      https.get(u, { headers: { 'User-Agent': 'noctune-install' } }, (res) => {
        if (res.statusCode === 301 || res.statusCode === 302) {
          res.resume();
          follow(new URL(res.headers.location, u).href, redirects + 1);
          return;
        }
        if (res.statusCode !== 200) {
          reject(new Error(`HTTP ${res.statusCode} downloading ${u}`));
          return;
        }
        const file = fs.createWriteStream(dest);
        res.on('error', reject);
        res.on('aborted', () => reject(new Error('Incomplete download')));
        res.pipe(file);
        file.on('finish', () => file.close(resolve));
        file.on('error', reject);
      }).on('error', reject).setTimeout(90000, function () { this.destroy(new Error('Download timed out')); });
    };
    follow(url);
  });
}

async function main() {
  const artifact = getArtifactName();
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${artifact}`;

  fs.mkdirSync(BIN_DIR, { recursive: true });

  console.log(`noctune: downloading ${artifact}...`);
  const staging = fs.mkdtempSync(path.join(BIN_DIR, '.install-'));
  const candidate = path.join(staging, artifact);
  const checksum = path.join(staging, 'checksum');
  try {
    await download(url, candidate);
    await download(url + '.sha256', checksum);
    const fields = fs.readFileSync(checksum, 'utf8').trim().split(/\s+/);
    const actual = crypto.createHash('sha256').update(fs.readFileSync(candidate)).digest('hex');
    if (fields.length !== 2 || fields[1] !== artifact || !/^[a-f0-9]{64}$/i.test(fields[0]) || fields[0].toLowerCase() !== actual) {
      throw new Error('Checksum mismatch; installation preserved');
    }
    if (process.platform !== 'win32') fs.chmodSync(candidate, 0o755);
    const backup = BIN_PATH + '.' + crypto.randomUUID() + '.old';
    const existed = fs.existsSync(BIN_PATH);
    if (existed) fs.renameSync(BIN_PATH, backup);
    try { fs.renameSync(candidate, BIN_PATH); }
    catch (error) { if (existed) fs.renameSync(backup, BIN_PATH); throw error; }
  } finally {
    for (const file of [candidate, checksum]) { if (fs.existsSync(file)) fs.unlinkSync(file); }
    fs.rmdirSync(staging);
  }

  console.log('noctune: installed successfully. Run: noctune');
}

main().catch((err) => {
  console.error('noctune: install failed:', err.message);
  process.exit(1);
});
