use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

use crate::errors::{Result, UpmError};

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

pub fn verify_checksum(path: &Path, expected: &str) -> Result<()> {
    let actual = sha256_file(path)?;
    if actual != expected {
        return Err(UpmError::VerificationFailed {
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(())
}

pub fn verify_directory(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(UpmError::General(format!("Path does not exist: {}", path.display())));
    }
    if !path.is_dir() {
        return Err(UpmError::General(format!("Path is not a directory: {}", path.display())));
    }
    let metadata = std::fs::metadata(path)?;
    if metadata.permissions().readonly() {
        return Err(UpmError::PermissionDenied(format!("Directory is read-only: {}", path.display())));
    }
    Ok(())
}
