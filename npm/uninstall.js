const { existsSync, rmSync } = require('fs');
const { join } = require('path');

const BINARY_NAME = process.platform === 'win32' ? 'upm.exe' : 'upm';
const BINARY_PATH = join(__dirname, 'bin', BINARY_NAME);

if (existsSync(BINARY_PATH)) {
  rmSync(BINARY_PATH);
  console.log('UPM binary removed.');
}
