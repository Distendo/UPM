// Encrypted Groq API key (XOR with embedded key)
// This is a shared/public key for all UPM users.
// To regenerate: python3 scripts/encrypt_groq_key.py "your_key_here"

const XOR_KEY: &[u8] = b"UpM_S3cr3t_K3y_2024!@#";

pub const ENCRYPTED_GROQ_KEY: &[u8] = &[
    50, 3, 38, 0, 33, 81, 1, 58, 122, 28, 56, 126, 125, 56, 102, 1, 0, 75, 96, 74, 117, 103,
    101, 24, 26, 24, 55, 74, 1, 65, 117, 45, 21, 51, 68, 49, 50, 11, 8, 87, 2, 25, 6, 84, 61,
    71, 15, 24, 30, 97, 2, 22, 84, 50, 10, 56,
];

pub fn decrypt_groq_key() -> Option<String> {
    if ENCRYPTED_GROQ_KEY.is_empty() {
        return None;
    }
    let decrypted: Vec<u8> = ENCRYPTED_GROQ_KEY
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ XOR_KEY[i % XOR_KEY.len()])
        .collect();
    String::from_utf8(decrypted).ok()
}
