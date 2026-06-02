use thiserror::Error;

#[derive(Error, Debug)]
pub enum UpmError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Package not found: {0}")]
    PackageNotFound(String),

    #[error("Package already installed: {0}")]
    PackageAlreadyInstalled(String),

    #[error("Package not installed: {0}")]
    PackageNotInstalled(String),

    #[error("Manifest parse error: {0}")]
    ManifestParseError(String),

    #[error("Verification failed: expected {expected}, got {actual}")]
    VerificationFailed { expected: String, actual: String },

    #[error("Dependency resolution failed: {0}")]
    DependencyError(String),

    #[error("Build failed: {0}")]
    BuildFailed(String),

    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Git error: {0}")]
    GitError(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("{0}")]
    General(String),
}

impl From<reqwest::Error> for UpmError {
    fn from(e: reqwest::Error) -> Self {
        UpmError::Network(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, UpmError>;
