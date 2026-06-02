const { execSync } = require('child_process');
const { existsSync } = require('fs');
const { join } = require('path');

const BINARY_NAME = process.platform === 'win32' ? 'upm.exe' : 'upm';
const BINARY_PATH = join(__dirname, 'bin', BINARY_NAME);

if (existsSync(BINARY_PATH)) {
  try {
    require('fs').rmSync(BINARY_PATH);
    console.log('UPM binary removed.');
  } catch {}
}

try {
  execSync('cargo uninstall upm 2>/dev/null || true', { stdio: 'inherit' });
} catch {}
