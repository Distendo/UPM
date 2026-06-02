const { execSync } = require('child_process');
const { existsSync, mkdirSync, copyFileSync, chmodSync, rmSync } = require('fs');
const { join } = require('path');

const BINARY_NAME = process.platform === 'win32' ? 'upm.exe' : 'upm';
const BIN_DIR = join(__dirname, 'bin');
const BINARY_PATH = join(BIN_DIR, BINARY_NAME);
const ROOT = join(__dirname, '..');

async function install() {
  if (!existsSync(BIN_DIR)) mkdirSync(BIN_DIR, { recursive: true });

  try {
    const where = execSync(`which ${BINARY_NAME} 2>/dev/null || echo ""`, { encoding: 'utf8' }).trim();
    if (where) {
      copyFileSync(where, BINARY_PATH);
      chmodSync(BINARY_PATH, 0o755);
      console.log('UPM linked from system installation.');
      return;
    }
  } catch {}

  const manifest = join(ROOT, 'Cargo.toml');
  if (existsSync(manifest)) {
    console.log('Building UPM from source...');
    execSync('cargo build --release', { cwd: ROOT, stdio: 'inherit' });
    const built = join(ROOT, 'target', 'release', BINARY_NAME);
    if (existsSync(built)) {
      copyFileSync(built, BINARY_PATH);
      chmodSync(BINARY_PATH, 0o755);
      console.log('UPM built and installed.');
      return;
    }
  }

  console.log('Building from GitHub source...');
  const tmp = join(__dirname, '.build');
  if (!existsSync(tmp)) {
    execSync(`git clone https://github.com/Distendo/UPM.git "${tmp}"`, { stdio: 'inherit' });
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
