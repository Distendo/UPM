const { execSync } = require('child_process');
const { existsSync, mkdirSync, chmodSync } = require('fs');
const { join } = require('path');

const BIN_DIR = join(__dirname, 'bin');
const BINARY_NAME = process.platform === 'win32' ? 'upm.exe' : 'upm';
const BINARY_PATH = join(BIN_DIR, BINARY_NAME);

async function install() {
  if (!existsSync(BIN_DIR)) mkdirSync(BIN_DIR, { recursive: true });

  const check = execSync(`which ${BINARY_NAME} 2>/dev/null || echo ""`, { encoding: 'utf8' }).trim();
  if (check) {
    console.log('UPM already installed globally.');
    return;
  }

  const repoUrl = 'https://github.com/Distendo/UPM.git';
  const buildDir = join(__dirname, '.build');

  console.log('Building UPM from source...');
  if (!existsSync(buildDir)) {
    execSync(`git clone "${repoUrl}" "${buildDir}"`, { stdio: 'inherit' });
  }
  execSync('cargo build --release', { cwd: buildDir, stdio: 'inherit' });

  const srcBin = join(buildDir, 'target', 'release', BINARY_NAME);
  if (existsSync(srcBin)) {
    const fs = require('fs');
    fs.copyFileSync(srcBin, BINARY_PATH);
    chmodSync(BINARY_PATH, 0o755);
    fs.rmSync(buildDir, { recursive: true, force: true });
    console.log('UPM installed successfully via npm.');
  } else {
    console.error('Build failed: binary not found.');
    process.exit(1);
  }
}

install().catch((err) => {
  console.error('Installation failed:', err.message);
  process.exit(1);
});
