const { execSync } = require('child_process');
const { existsSync, mkdirSync, copyFileSync, chmodSync, rmSync, realpathSync, readFileSync } = require('fs');
const { join } = require('path');

const BINARY_NAME = process.platform === 'win32' ? 'upm.exe' : 'upm';
const BIN_DIR = join(__dirname, 'bin');
const BINARY_PATH = join(BIN_DIR, BINARY_NAME);
const ROOT = join(__dirname, '..');

function isBinary(file) {
  try {
    const buf = readFileSync(file);
    return buf[0] === 0x7f || buf[0] === 0xcf || buf[0] === 0xfe || buf[0] === 0xca || buf[0] === 0x4d;
  } catch { return false; }
}

function findSystemBinary() {
  try {
    const out = execSync(`which ${BINARY_NAME} 2>/dev/null || echo ""`, { encoding: 'utf8' }).trim();
    if (!out) return null;
    const real = realpathSync(out);
    return isBinary(real) ? real : null;
  } catch { return null; }
}

async function install() {
  if (!existsSync(BIN_DIR)) mkdirSync(BIN_DIR, { recursive: true });

  const system = findSystemBinary();
  if (system) {
    copyFileSync(system, BINARY_PATH);
    chmodSync(BINARY_PATH, 0o755);
    console.log('UPM linked from:', system);
    return;
  }

  const manifest = join(ROOT, 'Cargo.toml');
  if (existsSync(manifest)) {
    console.log('Building UPM from source...');
    execSync('cargo build --release', { cwd: ROOT, stdio: 'inherit' });
    const built = join(ROOT, 'target', 'release', BINARY_NAME);
    if (existsSync(built)) {
      copyFileSync(built, BINARY_PATH);
      chmodSync(BINARY_PATH, 0o755);
      console.log('UPM built successfully.');
      return;
    }
  }

  console.log('Cloning and building from GitHub...');
  const tmp = join(__dirname, '.build');
  if (!existsSync(tmp)) {
    execSync(`git clone --depth=1 https://github.com/Distendo/UPM.git "${tmp}"`, { stdio: 'inherit' });
  }
  execSync('cargo build --release', { cwd: tmp, stdio: 'inherit' });
  const bin = join(tmp, 'target', 'release', BINARY_NAME);
  copyFileSync(bin, BINARY_PATH);
  chmodSync(BINARY_PATH, 0o755);
  rmSync(tmp, { recursive: true, force: true });
  console.log('UPM installed from GitHub.');
}

install().catch((err) => {
  console.error('Installation failed:', err.message);
  process.exit(1);
});
