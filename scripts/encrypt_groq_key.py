#!/usr/bin/env python3
"""Encrypt a Groq API key for embedding in UPM source.

Usage:
    python3 scripts/encrypt_groq_key.py "gsk_your_key_here"
"""

import sys

XOR_KEY = b"UpM_S3cr3t_K3y_2024!@#"


def encrypt(api_key: str) -> list[int]:
    return [ord(api_key[i]) ^ XOR_KEY[i % len(XOR_KEY)] for i in range(len(api_key))]


def main():
    if len(sys.argv) < 2:
        print("Usage: encrypt_groq_key.py <api_key>", file=sys.stderr)
        sys.exit(1)

    api_key = sys.argv[1]
    encrypted = encrypt(api_key)

    print("// Encrypted Groq API key (XOR with embedded key)")
    print(f"// Generated from: {api_key[:8]}...")
    print(f"pub const ENCRYPTED_GROQ_KEY: &[u8] = &{encrypted};")
    print(f"pub const GROQ_KEY_LEN: usize = {len(api_key)};")


if __name__ == "__main__":
    main()
