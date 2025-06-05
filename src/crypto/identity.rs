use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Unique identifier for a peer, derived from their public key
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(String);

impl PeerId {
    pub fn from_verifying_key(verifying_key: &VerifyingKey) -> Self {
        let encoded = general_purpose::STANDARD.encode(verifying_key.to_bytes());
        Self(encoded)
    }

    /// Create a PeerId from a string (for validation and reconstruction)
    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert PeerId back to VerifyingKey for signature verification
    pub fn to_verifying_key(&self) -> Result<VerifyingKey> {
        let decoded_bytes = general_purpose::STANDARD
            .decode(&self.0)
            .context("Failed to decode PeerId base64")?;

        if decoded_bytes.len() != 32 {
            return Err(anyhow::anyhow!(
                "Invalid PeerId key length: expected 32 bytes, got {}",
                decoded_bytes.len()
            ));
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&decoded_bytes);

        VerifyingKey::from_bytes(&key_bytes)
            .map_err(|e| anyhow::anyhow!("Invalid verifying key: {}", e))
    }
}

impl std::fmt::Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Serialize, Deserialize)]

struct IdentityData {
    secret_key: String,
    public_key: String,
}

/// Cryptographic identity using Ed25519 keys
pub struct Identity {
    signing_key: SigningKey,
    peer_id: PeerId,
}

impl Identity {
    /// Generate a new random identity
    pub fn generate() -> Result<Self> {
        use rand::RngCore;
        let mut csprng = rand::rngs::OsRng;
        let mut secret_bytes = [0u8; 32];
        csprng.fill_bytes(&mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let peer_id = PeerId::from_verifying_key(&signing_key.verifying_key());
        Ok(Self {
            signing_key,
            peer_id,
        })
    }

    /// Load identity from default storage location  
    pub fn from_default_storage() -> Result<Self> {
        let path = crate::crypto::storage::default_key_path()
            .map_err(|e| anyhow::anyhow!("Storage error: {}", e))?;

        // Load using secure storage
        let content = crate::crypto::storage::load_key_secure(&path)
            .map_err(|e| anyhow::anyhow!("Storage error: {}", e))?;

        let content_str = String::from_utf8(content).context("Invalid UTF-8 in identity file")?;

        let data: IdentityData =
            serde_json::from_str(&content_str).context("Failed to parse identity file")?;
        let secret_bytes = general_purpose::STANDARD
            .decode(&data.secret_key)
            .context("Invalid secret key encoding")?;

        // Convert Vec<u8> to [u8; 32]
        if secret_bytes.len() != 32 {
            return Err(anyhow::anyhow!(
                "Invalid secret key length: expected 32 bytes, got {}",
                secret_bytes.len()
            ));
        }
        let mut secret_array = [0u8; 32];
        secret_array.copy_from_slice(&secret_bytes);

        let signing_key = SigningKey::from_bytes(&secret_array);
        let peer_id = PeerId::from_verifying_key(&signing_key.verifying_key());
        Ok(Self {
            signing_key,
            peer_id,
        })
    }

    /// Save identity to default storage location
    pub fn save_to_default_storage(&self) -> Result<()> {
        let path = crate::crypto::storage::default_key_path()
            .map_err(|e| anyhow::anyhow!("Storage error: {}", e))?;

        // Ensure directory exists
        crate::crypto::storage::ensure_directory_exists(&path)
            .map_err(|e| anyhow::anyhow!("Storage error: {}", e))?;

        // Serialize identity data
        let data = IdentityData {
            secret_key: general_purpose::STANDARD.encode(self.signing_key.to_bytes()),
            public_key: general_purpose::STANDARD
                .encode(self.signing_key.verifying_key().to_bytes()),
        };
        let json = serde_json::to_string_pretty(&data).context("Failed to serialize identity")?;

        // Save using secure storage
        crate::crypto::storage::save_key_secure(&path, json.as_bytes())
            .map_err(|e| anyhow::anyhow!("Storage error: {}", e))?;

        Ok(())
    }

    /// Load or generate identity using secure storage
    pub fn load_or_generate() -> Result<Self> {
        match Self::from_default_storage() {
            Ok(identity) => Ok(identity),
            Err(_) => {
                let identity = Self::generate()?;
                identity.save_to_default_storage()?;
                Ok(identity)
            }
        }
    }

    /// Get the peer ID for this identity
    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    /// Get the verifying (public) key
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    /// Verify a signature against a verifying key
    pub fn verify(verifying_key: &VerifyingKey, message: &[u8], signature: &Signature) -> bool {
        verifying_key.verify(message, signature).is_ok()
    }
}
