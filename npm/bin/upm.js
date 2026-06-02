#!/usr/bin/env node
const { spawnSync } = require('child_process');
const { existsSync, realpathSync, readFileSync } = require('fs');
const { join } = require('path');

const BINARY_NAME = process.platform === 'win32' ? 'upm.exe' : 'upm';

function isBinary(file) {
  try {
    const buf = readFileSync(file);
    // Mach-O (macOS) starts with 0xFEED, 0xCEFA, etc.
    // ELF (Linux) starts with 0x7F 'ELF'
    // Windows PE starts with 'MZ'
    return buf[0] === 0x7f || buf[0] === 0xcf || buf[0] === 0xfe || buf[0] === 0xca || buf[0] === 0x4d;
  } catch { return false; }
}

function findBinary() {
  const paths = process.env.PATH.split(require('path').delimiter);
  for (const dir of paths) {
    const candidate = join(dir, BINARY_NAME);
    if (existsSync(candidate)) {
      try {
        const real = realpathSync(candidate);
        if (isBinary(real)) return real;
      } catch {}
    }
  }
  return null;
}

const binary = findBinary();
if (!binary) {
  console.error('UPM binary not found. Install via: npm install github:Distendo/UPM');
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
process.exit(result.status ?? 1);
