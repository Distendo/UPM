#!/usr/bin/env node
const { spawnSync } = require('child_process');
const { existsSync } = require('fs');
const { join } = require('path');

const BINARY_NAME = process.platform === 'win32' ? 'upm.exe' : 'upm';

function findBinary() {
  const localBin = join(__dirname, BINARY_NAME);
  if (existsSync(localBin)) return localBin;

  const paths = process.env.PATH.split(require('path').delimiter);
  for (const dir of paths) {
    const candidate = join(dir, BINARY_NAME);
    if (existsSync(candidate)) return candidate;
  }
  return null;
}

const binary = findBinary();
if (!binary) {
  console.error('UPM is not installed. Run `cargo install upm` or download from https://github.com/Distendo/UPM');
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
process.exit(result.status ?? 1);
