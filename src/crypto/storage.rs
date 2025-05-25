use directories::ProjectDirs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use thiserror::Error;

// Enhanced error types for better error handling
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Failed to determine application directory")]
    DirectoryNotFound,
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Directory creation failed: {path}")]
    DirectoryCreationFailed { path: PathBuf },
    
    #[error("File operation failed: {operation} on {path}")]
    FileOperationFailed { operation: String, path: PathBuf },
    
    #[error("Invalid file permissions: expected {expected:o}, found {found:o}")]
    InvalidPermissions { expected: u32, found: u32 },
}

pub trait KeyStorage {
    fn default_key_path() -> Result<PathBuf, StorageError>;
    fn ensure_directory_exists(path: &PathBuf) -> Result<(), StorageError>;
}

pub struct DefaultKeyStorage;

impl KeyStorage for DefaultKeyStorage {
    fn default_key_path() -> Result<PathBuf, StorageError> {
        let proj_dirs = ProjectDirs::from("", "", "rust-chess")
            .ok_or(StorageError::DirectoryNotFound)?;
        
        let key_path = proj_dirs.data_dir().join("identity.key");
        Ok(key_path)
    }
    
    fn ensure_directory_exists(path: &PathBuf) -> Result<(), StorageError> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|_| StorageError::DirectoryCreationFailed { 
                        path: parent.to_path_buf() 
                    })?;
            }
        }
        Ok(())
    }
}

impl DefaultKeyStorage {
    /// Save key data with proper permissions and directory creation
    pub fn save_key_secure<P: AsRef<Path>>(path: P, data: &[u8]) -> Result<(), StorageError> {
        let path = path.as_ref();
        
        // Ensure parent directory exists
        Self::ensure_directory_exists(&path.to_path_buf())?;
        
        // Write file
        std::fs::write(path, data)
            .map_err(|_| StorageError::FileOperationFailed {
                operation: "write".to_string(),
                path: path.to_path_buf(),
            })?;
        
        // Set secure permissions (Unix only)
        #[cfg(unix)]
        Self::set_secure_permissions(path)?;
        
        Ok(())
    }
    
    /// Load key data from file
    pub fn load_key_secure<P: AsRef<Path>>(path: P) -> Result<Vec<u8>, StorageError> {
        let path = path.as_ref();
        
        // Verify permissions before reading (Unix only)
        #[cfg(unix)]
        Self::verify_secure_permissions(path)?;
        
        // Read file
        std::fs::read(path)
            .map_err(|_| StorageError::FileOperationFailed {
                operation: "read".to_string(),
                path: path.to_path_buf(),
            })
    }
    
    #[cfg(unix)]
    fn set_secure_permissions<P: AsRef<Path>>(path: P) -> Result<(), StorageError> {
        use std::os::unix::fs::PermissionsExt;
        
        let mut perms = std::fs::metadata(&path)
            .map_err(|_| StorageError::FileOperationFailed {
                operation: "get_permissions".to_string(),
                path: path.as_ref().to_path_buf(),
            })?
            .permissions();
        
        perms.set_mode(0o600);
        std::fs::set_permissions(&path, perms)
            .map_err(|_| StorageError::FileOperationFailed {
                operation: "set_permissions".to_string(),
                path: path.as_ref().to_path_buf(),
            })?;
        
        Ok(())
    }
    
    #[cfg(unix)]
    fn verify_secure_permissions<P: AsRef<Path>>(path: P) -> Result<(), StorageError> {
        use std::os::unix::fs::PermissionsExt;
        
        let metadata = std::fs::metadata(&path)
            .map_err(|_| StorageError::FileOperationFailed {
                operation: "get_metadata".to_string(),
                path: path.as_ref().to_path_buf(),
            })?;
        
        let mode = metadata.permissions().mode() & 0o777;
        if mode != 0o600 {
            return Err(StorageError::InvalidPermissions {
                expected: 0o600,
                found: mode,
            });
        }
        
        Ok(())
    }
    
    #[cfg(windows)]
    fn set_secure_permissions<P: AsRef<Path>>(_path: P) -> Result<(), StorageError> {
        // Windows permission handling would go here
        // For now, return Ok since Windows file permissions work differently
        Ok(())
    }
    
    #[cfg(windows)]
    fn verify_secure_permissions<P: AsRef<Path>>(_path: P) -> Result<(), StorageError> {
        // Windows permission verification would go here
        // For now, return Ok since Windows file permissions work differently
        Ok(())
    }
}

/// Get the default key storage path for the current platform
pub fn default_key_path() -> Result<PathBuf, StorageError> {
    DefaultKeyStorage::default_key_path()
}



/// Ensure the directory for the given path exists
pub fn ensure_directory_exists(path: &PathBuf) -> Result<(), StorageError> {
    DefaultKeyStorage::ensure_directory_exists(path)
}

/// Save key data with proper permissions and directory creation
pub fn save_key_secure<P: AsRef<Path>>(path: P, data: &[u8]) -> Result<(), StorageError> {
    DefaultKeyStorage::save_key_secure(path, data)
}

/// Load key data from file with permission verification
pub fn load_key_secure<P: AsRef<Path>>(path: P) -> Result<Vec<u8>, StorageError> {
    DefaultKeyStorage::load_key_secure(path)
}
