//! Public Key Infrastructure (PKI) Module
//!
//! Implements hierarchical key management for Oxifed with support for:
//! - Master keys (root of trust)
//! - Domain keys (per-domain authority)  
//! - User keys (individual identity)
//! - Instance actor keys (system operations)

use crate::httpsignature::SignatureAlgorithm;
use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use ring::signature::{Ed25519KeyPair, KeyPair as RingKeyPair};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use thiserror::Error;

/// PKI-related errors
#[derive(Error, Debug)]
pub enum PkiError {
    #[error("Key generation failed: {0}")]
    KeyGenerationError(String),

    #[error("Key parsing failed: {0}")]
    KeyParseError(String),

    #[error("Signature creation failed: {0}")]
    SignatureCreationError(String),

    #[error("Signature verification failed: {0}")]
    SignatureVerificationError(String),

    #[error("Trust chain validation failed: {0}")]
    TrustChainError(String),

    #[error("Key not found: {0}")]
    KeyNotFoundError(String),

    #[error("Domain verification failed: {0}")]
    DomainVerificationError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Base64 encoding error: {0}")]
    Base64Error(#[from] base64::DecodeError),

    #[error("Invalid key format")]
    InvalidKeyFormat,

    #[error("Unsupported algorithm: {0}")]
    UnsupportedAlgorithm(String),
}

/// Trust levels in the PKI hierarchy
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustLevel {
    /// Self-signed user key without domain verification
    Unverified = 0,
    /// Domain-signed user key (verified by domain authority)
    DomainVerified = 1,
    /// Master-signed domain key (root of trust)
    MasterSigned = 2,
    /// Instance actor key (server-level authority)
    InstanceActor = 3,
}

impl TrustLevel {
    /// Get cache TTL based on trust level
    pub fn cache_ttl(&self) -> chrono::Duration {
        match self {
            TrustLevel::Unverified => chrono::Duration::minutes(15),
            TrustLevel::DomainVerified => chrono::Duration::hours(4),
            TrustLevel::MasterSigned => chrono::Duration::hours(24),
            TrustLevel::InstanceActor => chrono::Duration::hours(12),
        }
    }

    /// Get rate limit multiplier based on trust level
    pub fn rate_limit_multiplier(&self) -> f64 {
        match self {
            TrustLevel::Unverified => 0.5,
            TrustLevel::DomainVerified => 1.0,
            TrustLevel::MasterSigned => 3.0,
            TrustLevel::InstanceActor => 2.0,
        }
    }
}

/// Cryptographic algorithm types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KeyAlgorithm {
    #[serde(rename = "RSA")]
    Rsa { key_size: u32 },
    #[serde(rename = "Ed25519")]
    Ed25519,
}

impl KeyAlgorithm {
    /// Convert to signature algorithm
    pub fn to_signature_algorithm(&self) -> SignatureAlgorithm {
        match self {
            KeyAlgorithm::Rsa { .. } => SignatureAlgorithm::RsaSha256,
            KeyAlgorithm::Ed25519 => SignatureAlgorithm::Ed25519,
        }
    }
}

/// Public key representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKey {
    pub algorithm: KeyAlgorithm,
    pub pem_data: String,
    pub fingerprint: String,
}

impl PublicKey {
    /// Create a new public key from PEM data
    pub fn from_pem(algorithm: KeyAlgorithm, pem_data: String) -> Result<Self, PkiError> {
        let fingerprint = Self::calculate_fingerprint(&pem_data)?;
        Ok(Self {
            algorithm,
            pem_data,
            fingerprint,
        })
    }

    /// Calculate SHA-256 fingerprint of the key
    fn calculate_fingerprint(pem_data: &str) -> Result<String, PkiError> {
        let mut hasher = Sha256::new();
        hasher.update(pem_data.as_bytes());
        let result = hasher.finalize();
        Ok(format!("sha256:{}", hex::encode(result)))
    }

    /// Get the key ID URL
    pub fn key_id(&self, actor_id: &str) -> String {
        format!("{}#main-key", actor_id)
    }
}

/// Private key representation (encrypted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateKey {
    pub algorithm: KeyAlgorithm,
    pub encrypted_pem: String,
    pub encryption_algorithm: String,
}

impl PrivateKey {
    /// Create a new private key from PEM data
    pub fn from_pem(algorithm: KeyAlgorithm, pem_data: String) -> Result<Self, PkiError> {
        // For now, store unencrypted but mark as needing encryption
        Ok(Self {
            algorithm,
            encrypted_pem: pem_data,
            encryption_algorithm: "none".to_string(),
        })
    }

    /// Decrypt and return the raw PEM data
    pub fn decrypt(&self, _passphrase: Option<&str>) -> Result<String, PkiError> {
        // TODO: Implement actual decryption
        Ok(self.encrypted_pem.clone())
    }
}

/// Key pair (public + private key)
#[derive(Debug, Clone)]
pub struct KeyPair {
    pub public_key: PublicKey,
    pub private_key: PrivateKey,
}

impl KeyPair {
    /// Create a key pair from PEM strings (for imported keys)
    pub fn from_pem(
        algorithm: KeyAlgorithm,
        public_pem: String,
        private_pem: String,
    ) -> Result<Self, PkiError> {
        Ok(Self {
            public_key: PublicKey::from_pem(algorithm.clone(), public_pem)?,
            private_key: PrivateKey::from_pem(algorithm, private_pem)?,
        })
    }

    /// Generate a new key pair
    pub fn generate(algorithm: KeyAlgorithm) -> Result<Self, PkiError> {
        match algorithm {
            KeyAlgorithm::Rsa { key_size } => Self::generate_rsa(key_size),
            KeyAlgorithm::Ed25519 => Self::generate_ed25519(),
        }
    }

    /// Generate an RSA key pair
    fn generate_rsa(key_size: u32) -> Result<Self, PkiError> {
        use pkcs8::EncodePrivateKey;

        let mut rng = rand::rngs::OsRng;
        let bits = key_size as usize;
        let private_key = rsa::RsaPrivateKey::new(&mut rng, bits).map_err(|e| {
            PkiError::KeyGenerationError(format!("RSA key generation failed: {}", e))
        })?;

        let private_pem = private_key
            .to_pkcs8_pem(pkcs8::LineEnding::LF)
            .map_err(|e| {
                PkiError::KeyGenerationError(format!("Failed to encode RSA private key: {}", e))
            })?
            .to_string();

        let public_key = private_key.to_public_key();

        // Mastodon and ActivityPub expect SPKI "PUBLIC KEY" format, not PKCS#1 "RSA PUBLIC KEY"
        use rsa::pkcs8::EncodePublicKey;
        let public_pem_spki = public_key
            .to_public_key_pem(pkcs8::LineEnding::LF)
            .map_err(|e| {
                PkiError::KeyGenerationError(format!("Failed to encode RSA SPKI key: {}", e))
            })?;

        Self::from_pem(KeyAlgorithm::Rsa { key_size }, public_pem_spki, private_pem)
    }

    /// Generate an Ed25519 key pair using the ring crate
    fn generate_ed25519() -> Result<Self, PkiError> {
        let rng = ring::rand::SystemRandom::new();
        let pkcs8_doc = Ed25519KeyPair::generate_pkcs8(&rng).map_err(|e| {
            PkiError::KeyGenerationError(format!("Ed25519 generation failed: {}", e))
        })?;

        let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8_doc.as_ref()).map_err(|e| {
            PkiError::KeyGenerationError(format!("Failed to parse generated key: {}", e))
        })?;

        // Encode private key as PEM (PKCS#8 DER -> PEM)
        let private_pem = der_to_pem(pkcs8_doc.as_ref(), "PRIVATE KEY");

        // Ed25519 public key: the raw 32-byte key needs to be wrapped in
        // SubjectPublicKeyInfo DER structure for standard PEM encoding
        let public_key_bytes = key_pair.public_key().as_ref();
        let public_key_der = encode_ed25519_spki(public_key_bytes);
        let public_pem = der_to_pem(&public_key_der, "PUBLIC KEY");

        Self::from_pem(KeyAlgorithm::Ed25519, public_pem, private_pem)
    }

    /// Sign data with the private key
    pub fn sign(&self, data: &[u8]) -> Result<String, PkiError> {
        match &self.private_key.algorithm {
            KeyAlgorithm::Ed25519 => {
                let private_der = pem_to_der(&self.private_key.encrypted_pem)
                    .map_err(|e| PkiError::SignatureCreationError(format!("Invalid PEM: {}", e)))?;
                let key_pair = Ed25519KeyPair::from_pkcs8(&private_der).map_err(|e| {
                    PkiError::SignatureCreationError(format!("Invalid Ed25519 key: {}", e))
                })?;
                let sig = key_pair.sign(data);
                Ok(general_purpose::STANDARD.encode(sig.as_ref()))
            }
            KeyAlgorithm::Rsa { .. } => Err(PkiError::UnsupportedAlgorithm(
                "RSA signing not supported in PKI module. Use httpsignature module.".to_string(),
            )),
        }
    }
}

/// Encode DER bytes as PEM with the given label (e.g. "PRIVATE KEY", "PUBLIC KEY")
fn der_to_pem(der: &[u8], label: &str) -> String {
    let b64 = general_purpose::STANDARD.encode(der);
    let mut pem = format!("-----BEGIN {}-----\n", label);
    for chunk in b64.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(chunk).unwrap());
        pem.push('\n');
    }
    pem.push_str(&format!("-----END {}-----", label));
    pem
}

/// Decode PEM to DER bytes
fn pem_to_der(pem: &str) -> Result<Vec<u8>, PkiError> {
    let lines: Vec<&str> = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    general_purpose::STANDARD
        .decode(lines.join(""))
        .map_err(|e| PkiError::KeyParseError(format!("Base64 decode failed: {}", e)))
}

/// Encode raw Ed25519 public key bytes into SubjectPublicKeyInfo DER
fn encode_ed25519_spki(public_key: &[u8]) -> Vec<u8> {
    // SubjectPublicKeyInfo for Ed25519:
    // SEQUENCE {
    //   SEQUENCE { OID 1.3.101.112 }
    //   BIT STRING (public key)
    // }
    let oid: &[u8] = &[0x06, 0x03, 0x2b, 0x65, 0x70]; // OID 1.3.101.112
    let algo_seq_len = oid.len();
    let algo_seq: Vec<u8> = [&[0x30, algo_seq_len as u8], oid].concat();
    let bit_string: Vec<u8> = [&[0x03, (public_key.len() + 1) as u8, 0x00], public_key].concat();
    let total_len = algo_seq.len() + bit_string.len();
    [
        &[0x30, total_len as u8],
        algo_seq.as_slice(),
        bit_string.as_slice(),
    ]
    .concat()
}

/// Domain signature (used to sign user keys)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainSignature {
    pub domain: String,
    pub signature: String,
    pub signed_at: DateTime<Utc>,
    pub domain_key_id: String,
    pub verification_chain: Vec<String>,
}

/// User key information with trust metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserKeyInfo {
    pub actor_id: String,
    pub key_id: String,
    pub public_key: PublicKey,
    pub private_key: Option<PrivateKey>,
    pub domain_signature: Option<DomainSignature>,
    pub trust_level: TrustLevel,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub rotation_policy: RotationPolicy,
}

impl UserKeyInfo {
    /// Create a new unverified user key
    pub fn new_unverified(actor_id: String, key_pair: KeyPair) -> Self {
        let key_id = key_pair.public_key.key_id(&actor_id);

        Self {
            actor_id,
            key_id,
            public_key: key_pair.public_key,
            private_key: Some(key_pair.private_key),
            domain_signature: None,
            trust_level: TrustLevel::Unverified,
            created_at: Utc::now(),
            expires_at: None,
            rotation_policy: RotationPolicy::default(),
        }
    }

    /// Upgrade trust level with domain signature
    pub fn upgrade_trust(&mut self, domain_signature: DomainSignature) {
        self.domain_signature = Some(domain_signature);
        self.trust_level = TrustLevel::DomainVerified;
    }

    /// Check if key is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }
}

/// Key rotation policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationPolicy {
    pub automatic: bool,
    pub rotation_interval: Option<chrono::Duration>,
    pub max_age: Option<chrono::Duration>,
    pub notify_before: Option<chrono::Duration>,
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            automatic: false,
            rotation_interval: Some(chrono::Duration::days(365)), // 1 year
            max_age: Some(chrono::Duration::days(400)),           // 13 months max
            notify_before: Some(chrono::Duration::days(30)),      // 30 days notice
        }
    }
}

/// Master key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterKeyInfo {
    pub key_id: String,
    pub public_key: PublicKey,
    pub private_key: PrivateKey,
    pub created_at: DateTime<Utc>,
    pub fingerprint: String,
    pub usage: Vec<KeyUsage>,
}

/// Domain key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainKeyInfo {
    pub domain: String,
    pub key_id: String,
    pub public_key: PublicKey,
    pub private_key: PrivateKey,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub master_signature: Option<MasterSignature>,
    pub usage: Vec<KeyUsage>,
}

/// Master signature (used to sign domain keys)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterSignature {
    pub signature: String,
    pub signed_at: DateTime<Utc>,
    pub master_key_id: String,
}

/// Key usage types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KeyUsage {
    #[serde(rename = "domain-signing")]
    DomainSigning,
    #[serde(rename = "user-signing")]
    UserSigning,
    #[serde(rename = "instance-actor")]
    InstanceActor,
    #[serde(rename = "emergency-recovery")]
    EmergencyRecovery,
}

/// Trust chain link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustChainLink {
    pub level: String,
    pub key_id: String,
    pub signed_by: Option<String>,
    pub signed_at: Option<DateTime<Utc>>,
    pub self_signed: bool,
    pub created_at: DateTime<Utc>,
}

/// Complete trust chain for a key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustChain {
    pub key_id: String,
    pub trust_level: TrustLevel,
    pub verification_chain: Vec<TrustChainLink>,
    pub verified_at: DateTime<Utc>,
}

/// PKI Manager - main interface for key operations
pub struct PkiManager {
    pub master_key: Option<MasterKeyInfo>,
    pub domain_keys: HashMap<String, DomainKeyInfo>,
    pub user_keys: HashMap<String, UserKeyInfo>,
    pub instance_keys: HashMap<String, UserKeyInfo>,
}

impl PkiManager {
    /// Create a new PKI manager
    pub fn new() -> Self {
        Self {
            master_key: None,
            domain_keys: HashMap::new(),
            user_keys: HashMap::new(),
            instance_keys: HashMap::new(),
        }
    }

    /// Generate a new key pair for a user
    pub fn generate_user_key(
        &mut self,
        actor_id: String,
        algorithm: KeyAlgorithm,
    ) -> Result<UserKeyInfo, PkiError> {
        // Generate a new key pair
        let key_pair = KeyPair::generate(algorithm).map_err(|e| {
            PkiError::KeyGenerationError(format!("Failed to generate key pair: {}", e))
        })?;

        // Create a new user key with the generated key pair
        let user_key = UserKeyInfo::new_unverified(actor_id.clone(), key_pair);
        self.user_keys.insert(actor_id, user_key.clone());

        Ok(user_key)
    }

    /// Import user key (BYOK - Bring Your Own Key)
    pub fn import_user_key(
        &mut self,
        actor_id: String,
        key_pair: KeyPair,
    ) -> Result<UserKeyInfo, PkiError> {
        let user_key = UserKeyInfo::new_unverified(actor_id.clone(), key_pair);
        self.user_keys.insert(actor_id, user_key.clone());
        Ok(user_key)
    }

    /// Verify and sign user key with domain authority
    pub fn verify_and_sign_user_key(
        &mut self,
        actor_id: &str,
        domain: &str,
    ) -> Result<(), PkiError> {
        let domain_key = self.domain_keys.get(domain).ok_or_else(|| {
            PkiError::KeyNotFoundError(format!("Domain key for {} not found", domain))
        })?;

        let user_key = self.user_keys.get_mut(actor_id).ok_or_else(|| {
            PkiError::KeyNotFoundError(format!("User key for {} not found", actor_id))
        })?;

        // Create domain signature
        let signature_data = format!("{}:{}", user_key.key_id, user_key.public_key.fingerprint);
        let domain_key_pair = KeyPair {
            public_key: domain_key.public_key.clone(),
            private_key: domain_key.private_key.clone(),
        };
        let signature = domain_key_pair.sign(signature_data.as_bytes())?;

        let domain_signature = DomainSignature {
            domain: domain.to_string(),
            signature,
            signed_at: Utc::now(),
            domain_key_id: domain_key.key_id.clone(),
            verification_chain: vec![domain_key.key_id.clone()],
        };

        user_key.upgrade_trust(domain_signature);
        Ok(())
    }

    /// Build trust chain for a key
    pub fn build_trust_chain(&self, key_id: &str) -> Result<TrustChain, PkiError> {
        // Find the key
        let user_key = self
            .user_keys
            .values()
            .find(|uk| uk.key_id == key_id)
            .ok_or_else(|| PkiError::KeyNotFoundError(format!("Key {} not found", key_id)))?;

        let mut chain = Vec::new();

        // Add user key link
        let user_link = TrustChainLink {
            level: "user".to_string(),
            key_id: user_key.key_id.clone(),
            signed_by: user_key
                .domain_signature
                .as_ref()
                .map(|ds| ds.domain_key_id.clone()),
            signed_at: user_key.domain_signature.as_ref().map(|ds| ds.signed_at),
            self_signed: user_key.domain_signature.is_none(),
            created_at: user_key.created_at,
        };
        chain.push(user_link);

        // Add domain key link if exists
        if let Some(domain_sig) = &user_key.domain_signature
            && let Some(domain_key) = self.domain_keys.get(&domain_sig.domain)
        {
            let domain_link = TrustChainLink {
                level: "domain".to_string(),
                key_id: domain_key.key_id.clone(),
                signed_by: domain_key
                    .master_signature
                    .as_ref()
                    .map(|ms| ms.master_key_id.clone()),
                signed_at: domain_key.master_signature.as_ref().map(|ms| ms.signed_at),
                self_signed: domain_key.master_signature.is_none(),
                created_at: domain_key.created_at,
            };
            chain.push(domain_link);
        }

        // Add master key link if exists
        if let Some(master_key) = &self.master_key {
            let master_link = TrustChainLink {
                level: "master".to_string(),
                key_id: master_key.key_id.clone(),
                signed_by: None,
                signed_at: None,
                self_signed: true,
                created_at: master_key.created_at,
            };
            chain.push(master_link);
        }

        Ok(TrustChain {
            key_id: key_id.to_string(),
            trust_level: user_key.trust_level,
            verification_chain: chain,
            verified_at: Utc::now(),
        })
    }

    /// Get user key by actor ID
    pub fn get_user_key(&self, actor_id: &str) -> Option<&UserKeyInfo> {
        self.user_keys.get(actor_id)
    }

    /// Get domain key by domain
    pub fn get_domain_key(&self, domain: &str) -> Option<&DomainKeyInfo> {
        self.domain_keys.get(domain)
    }

    /// Validate trust chain for a key
    pub fn validate_trust_chain(&self, key_id: &str) -> Result<TrustLevel, PkiError> {
        let trust_chain = self.build_trust_chain(key_id)?;

        // Verify each link in the chain
        for link in trust_chain.verification_chain.iter() {
            if !link.self_signed
                && let Some(signer_key_id) = &link.signed_by
            {
                // Verify signature exists and is valid
                // This would involve cryptographic verification in a real implementation
                tracing::debug!(
                    "Verifying signature from {} for {}",
                    signer_key_id,
                    link.key_id
                );
            }
        }

        Ok(trust_chain.trust_level)
    }
}

impl Default for PkiManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_key_import() {
        let mut pki_manager = PkiManager::new();

        let key_pair = KeyPair::from_pem(
            KeyAlgorithm::Rsa { key_size: 2048 },
            "-----BEGIN PUBLIC KEY-----\ntest\n-----END PUBLIC KEY-----".to_string(),
            "-----BEGIN PRIVATE KEY-----\ntest\n-----END PRIVATE KEY-----".to_string(),
        )
        .unwrap();

        let actor_id = "https://example.com/users/alice".to_string();
        let user_key = pki_manager
            .import_user_key(actor_id.clone(), key_pair)
            .unwrap();

        assert_eq!(user_key.trust_level, TrustLevel::Unverified);
        assert_eq!(user_key.actor_id, actor_id);
    }

    #[test]
    fn test_trust_levels() {
        assert!(TrustLevel::InstanceActor > TrustLevel::MasterSigned);
        assert!(TrustLevel::MasterSigned > TrustLevel::DomainVerified);
        assert!(TrustLevel::DomainVerified > TrustLevel::Unverified);
    }

    #[test]
    fn test_ed25519_generate_and_sign_verify() {
        let key_pair = KeyPair::generate(KeyAlgorithm::Ed25519).unwrap();

        // Verify PEM headers
        assert!(
            key_pair
                .public_key
                .pem_data
                .starts_with("-----BEGIN PUBLIC KEY-----")
        );
        assert!(
            key_pair
                .private_key
                .encrypted_pem
                .starts_with("-----BEGIN PRIVATE KEY-----")
        );
        assert!(key_pair.public_key.fingerprint.starts_with("sha256:"));

        // Sign and verify round-trip
        let data = b"hello world";
        let signature_b64 = key_pair.sign(data).unwrap();

        // Decode signature and verify using ring
        let sig_bytes = general_purpose::STANDARD.decode(&signature_b64).unwrap();
        let pub_der = pem_to_der(&key_pair.public_key.pem_data).unwrap();
        // Extract raw 32-byte public key from SPKI DER (offset 12)
        let raw_pub = &pub_der[12..44];
        let verify_key =
            ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, raw_pub);
        verify_key.verify(data, &sig_bytes).unwrap();
    }

    #[test]
    fn test_rsa_generate() {
        let result = KeyPair::generate(KeyAlgorithm::Rsa { key_size: 2048 });
        assert!(result.is_ok());
        let key_pair = result.unwrap();
        assert!(key_pair.public_key.pem_data.contains("BEGIN PUBLIC KEY"));
        assert!(matches!(
            key_pair.public_key.algorithm,
            KeyAlgorithm::Rsa { key_size: 2048 }
        ));
    }

    #[test]
    fn test_cache_ttl() {
        assert_eq!(
            TrustLevel::Unverified.cache_ttl(),
            chrono::Duration::minutes(15)
        );
        assert_eq!(
            TrustLevel::DomainVerified.cache_ttl(),
            chrono::Duration::hours(4)
        );
        assert_eq!(
            TrustLevel::MasterSigned.cache_ttl(),
            chrono::Duration::hours(24)
        );
        assert_eq!(
            TrustLevel::InstanceActor.cache_ttl(),
            chrono::Duration::hours(12)
        );
    }
}
