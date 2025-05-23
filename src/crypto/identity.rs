
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use anyhow::{Result, Context};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::os::unix::fs::PermissionsExt;


/// Unique identifier for a peer, derived from their public key

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(String);


impl PeerId {
    pub fn from_verifying_key(verifying_key: &VerifyingKey) -> Self {
        let encoded = general_purpose::STANDARD.encode(verifying_key.to_bytes());
        Self(encoded)
    }

    pub fn as_str(&self) -> &str {
        &self.0
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
        let mut csprng = rand::rngs::OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let peer_id = PeerId::from_verifying_key(&signing_key.verifying_key());
        Ok(Self { signing_key, peer_id })
    }


    /// Load identity from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .context("Failed to read identity file")?;
        let data: IdentityData = serde_json::from_str(&content)
            .context("Failed to parse identity file")?;
        let secret_bytes = general_purpose::STANDARD.decode(&data.secret_key)
            .context("Invalid secret key encoding")?;
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let peer_id = PeerId::from_verifying_key(&signing_key.verifying_key());
        Ok(Self { signing_key, peer_id })
    }


    /// Save identity to file with secure permissions (0600)
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let data = IdentityData {
            secret_key: general_purpose::STANDARD.encode(self.signing_key.to_bytes()),
            public_key: general_purpose::STANDARD.encode(self.signing_key.verifying_key().to_bytes()),
        };
        let json = serde_json::to_string_pretty(&data)
            .context("Failed to serialize identity")?;
        fs::write(path.as_ref(), json)
            .context("Failed to write identity file")?;
        // Set secure permissions (owner read/write only)
        let mut perms = fs::metadata(path.as_ref())?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path.as_ref(), perms)
            .context("Failed to set file permissions")?;
        Ok(())
    }

    /// Get the peer ID for this identity
    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    /// Get the verifying (public) key
    pub fn public_key(&self) -> VerifyingKey {
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
