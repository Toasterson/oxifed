//! HTTP Signature implementation based on RFC 9421.
//!
//! This module provides functionality for creating and verifying HTTP signatures
//! according to the specifications in RFC 9421 (https://www.rfc-editor.org/rfc/rfc9421.html).
//!
//! The module supports various signing algorithms, key types, and signature parameters
//! as specified in the standard.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::{DateTime, Duration, Utc};
use reqwest::{header::{HeaderMap, HeaderName, HeaderValue}, Method, Request, Response};
use regex::Regex;
use ring::signature::{self, EcdsaKeyPair, Ed25519KeyPair, KeyPair, RsaKeyPair};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use thiserror::Error;
use url::Url;

/// Algorithm supported for HTTP signatures
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SignatureAlgorithm {
    /// RSASSA-PKCS1-v1_5 using SHA-256
    RsaSha256,
    /// RSASSA-PSS using SHA-512
    RsaPssSha512,
    /// ECDSA using curve P-256 DSS and SHA-256
    EcdsaP256Sha256,
    /// EdDSA using curve Ed25519
    Ed25519,
}

impl SignatureAlgorithm {
    /// Returns the string identifier for the algorithm as defined in RFC 9421
    pub fn as_str(&self) -> &'static str {
        match self {
            SignatureAlgorithm::RsaSha256 => "rsa-v1_5-sha256",
            SignatureAlgorithm::RsaPssSha512 => "rsa-pss-sha512",
            SignatureAlgorithm::EcdsaP256Sha256 => "ecdsa-p256-sha256",
            SignatureAlgorithm::Ed25519 => "ed25519",
        }
    }
}

impl FromStr for SignatureAlgorithm {
    type Err = SignatureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "rsa-v1_5-sha256" => Ok(SignatureAlgorithm::RsaSha256),
            "rsa-pss-sha512" => Ok(SignatureAlgorithm::RsaPssSha512),
            "ecdsa-p256-sha256" => Ok(SignatureAlgorithm::EcdsaP256Sha256),
            "ed25519" => Ok(SignatureAlgorithm::Ed25519),
            _ => Err(SignatureError::UnsupportedAlgorithm(s.to_string())),
        }
    }
}

/// Error types for HTTP signature operations
#[derive(Debug, Error)]
pub enum SignatureError {
    #[error("Invalid signature parameter: {0}")]
    InvalidParameter(String),
    
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),
    
    #[error("Unsupported algorithm: {0}")]
    UnsupportedAlgorithm(String),
    
    #[error("Invalid key format: {0}")]
    InvalidKeyFormat(String),
    
    #[error("Signature verification failed")]
    VerificationFailed,
    
    #[error("Signature expired")]
    SignatureExpired,
    
    #[error("Signature creation date in the future")]
    SignatureCreatedInFuture,
    
    #[error("Cryptographic operation failed: {0}")]
    CryptoError(String),
    
    #[error("Base64 encoding/decoding error: {0}")]
    Base64Error(#[from] base64::DecodeError),
    
    #[error("Invalid HTTP header: {0}")]
    InvalidHeader(String),
    
    #[error("Invalid signature input: {0}")]
    InvalidSignatureInput(String),
    
    #[error("Request error: {0}")]
    RequestError(String),
}

/// Component identifier for HTTP message components
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ComponentIdentifier {
    /// HTTP message header, lowercase, standard or extension
    Header(String),
    
    /// HTTP method, uppercase
    Method,
    
    /// Target URI
    TargetUri,
    
    /// Request target, original form of URI
    RequestTarget,
    
    /// HTTP request path, including query string
    Path,
    
    /// HTTP request query parameters
    Query,
    
    /// HTTP status code
    Status,
    
    /// Request and response body digests
    Digest,
}

impl ComponentIdentifier {
    /// Get the canonical name of the component as used in the signature base
    pub fn canonical_name(&self) -> String {
        match self {
            ComponentIdentifier::Header(name) => name.to_lowercase(),
            ComponentIdentifier::Method => "@method".to_string(),
            ComponentIdentifier::TargetUri => "@target-uri".to_string(),
            ComponentIdentifier::RequestTarget => "@request-target".to_string(),
            ComponentIdentifier::Path => "@path".to_string(),
            ComponentIdentifier::Query => "@query".to_string(),
            ComponentIdentifier::Status => "@status".to_string(),
            ComponentIdentifier::Digest => "digest".to_string(),
        }
    }
}

impl FromStr for ComponentIdentifier {
    type Err = SignatureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with('@') {
            match s {
                "@method" => Ok(ComponentIdentifier::Method),
                "@target-uri" => Ok(ComponentIdentifier::TargetUri),
                "@request-target" => Ok(ComponentIdentifier::RequestTarget),
                "@path" => Ok(ComponentIdentifier::Path),
                "@query" => Ok(ComponentIdentifier::Query),
                "@status" => Ok(ComponentIdentifier::Status),
                _ => Err(SignatureError::InvalidParameter(format!("Unknown derived component: {}", s))),
            }
        } else {
            Ok(ComponentIdentifier::Header(s.to_lowercase()))
        }
    }
}

/// Configuration for HTTP signature creation
#[derive(Debug, Clone)]
pub struct SignatureConfig {
    /// Signing algorithm to use
    pub algorithm: SignatureAlgorithm,
    
    /// Signature parameters to include
    pub parameters: SignatureParameters,
    
    /// Key ID for identifying the key
    pub key_id: String,
    
    /// Components to include in the signature
    pub components: Vec<ComponentIdentifier>,
    
    /// Private key for signing
    pub private_key: Vec<u8>,
}

/// Parameters for HTTP signature
#[derive(Debug, Clone, Default)]
pub struct SignatureParameters {
    /// Time when the signature was created
    pub created: Option<DateTime<Utc>>,
    
    /// Time when the signature expires
    pub expires: Option<DateTime<Utc>>,
    
    /// Nonce value for preventing replay attacks
    pub nonce: Option<String>,
    
    /// Key ID for identifying the key
    pub key_id: Option<String>,
    
    /// Tag for identifying this signature among multiple signatures
    pub tag: Option<String>,
    
    /// Algorithm used for the signature
    pub algorithm: Option<SignatureAlgorithm>,
}

impl SignatureParameters {
    /// Create a new signature parameters set with default values
    pub fn new() -> Self {
        Self {
            created: Some(Utc::now()),
            expires: Some(Utc::now() + Duration::hours(1)),
            ..Default::default()
        }
    }

    /// Parse signature parameters from a signature input string
    pub fn from_input(input: &str) -> Result<Self, SignatureError> {
        let mut params = Self::default();
        let input_regex = Regex::new(r#"(?:([a-zA-Z0-9_-]+)=(?:"([^"]*)"|([^;,\s]*)))(?:;|$|\s*,)"#).unwrap();
        
        for cap in input_regex.captures_iter(input) {
            let key = cap.get(1).unwrap().as_str();
            let value = cap.get(2).or_else(|| cap.get(3)).unwrap().as_str();
            
            match key {
                "created" => {
                    let timestamp = value.parse::<i64>()
                        .map_err(|_| SignatureError::InvalidParameter(format!("Invalid created timestamp: {}", value)))?;
                    params.created = Some(DateTime::from_timestamp(timestamp, 0)
                        .ok_or_else(|| SignatureError::InvalidParameter(format!("Invalid created timestamp: {}", value)))?);
                },
                "expires" => {
                    let timestamp = value.parse::<i64>()
                        .map_err(|_| SignatureError::InvalidParameter(format!("Invalid expires timestamp: {}", value)))?;
                    params.expires = Some(DateTime::from_timestamp(timestamp, 0)
                        .ok_or_else(|| SignatureError::InvalidParameter(format!("Invalid expires timestamp: {}", value)))?);
                },
                "nonce" => params.nonce = Some(value.to_string()),
                "keyid" => params.key_id = Some(value.to_string()),
                "tag" => params.tag = Some(value.to_string()),
                "alg" => params.algorithm = Some(SignatureAlgorithm::from_str(value)?),
                _ => return Err(SignatureError::InvalidParameter(format!("Unknown parameter: {}", key))),
            }
        }
        
        Ok(params)
    }
    
    /// Format parameters for inclusion in a signature input
    pub fn format_parameters(&self) -> String {
        let mut params = Vec::new();
        
        if let Some(created) = self.created {
            params.push(format!("created={}", created.timestamp()));
        }
        
        if let Some(expires) = self.expires {
            params.push(format!("expires={}", expires.timestamp()));
        }
        
        if let Some(nonce) = &self.nonce {
            params.push(format!("nonce=\"{}\"", nonce));
        }
        
        if let Some(key_id) = &self.key_id {
            params.push(format!("keyid=\"{}\"", key_id));
        }
        
        if let Some(algorithm) = &self.algorithm {
            params.push(format!("alg=\"{}\"", algorithm.as_str()));
        }
        
        params.join(";")
    }
}

/// HTTP Signature implementation
pub struct HttpSignature;

impl HttpSignature {
    /// Create a signature base for a request
    pub fn create_signature_base(
        req: &Request,
        components: &[ComponentIdentifier],
        params: &SignatureParameters,
    ) -> Result<String, SignatureError> {
        let mut base_lines = Vec::new();
        
        // Add components to signature base
        for component in components {
            let (name, value) = Self::get_component_value(req, component)?;
            base_lines.push(format!("\"{}\":{}", name, value));
        }
        
        // Add parameters
        base_lines.push(format!("\"@signature-params\":{}", params.format_parameters()));
        
        Ok(base_lines.join("\n"))
    }
    
    /// Get a component value from a request
    fn get_component_value(
        req: &Request,
        component: &ComponentIdentifier,
    ) -> Result<(String, String), SignatureError> {
        let name = component.canonical_name();
        
        match component {
            ComponentIdentifier::Header(header_name) => {
                let header = HeaderName::from_str(header_name)
                    .map_err(|_| SignatureError::InvalidHeader(header_name.clone()))?;
                
                if let Some(value) = req.headers().get(&header) {
                    let value_str = value.to_str()
                        .map_err(|_| SignatureError::InvalidHeader(format!("Non-ASCII value in header: {}", header_name)))?;
                    Ok((name, format!(" {}", value_str)))
                } else {
                    Err(SignatureError::InvalidHeader(format!("Header not found: {}", header_name)))
                }
            },
            ComponentIdentifier::Method => {
                Ok((name, format!(" {}", req.method().as_str())))
            },
            ComponentIdentifier::TargetUri => {
                let url = req.url();
                Ok((name, format!(" {}", url)))
            },
            ComponentIdentifier::RequestTarget => {
                let url = req.url();
                let path_and_query = if let Some(query) = url.query() {
                    format!("{}?{}", url.path(), query)
                } else {
                    url.path().to_string()
                };
                
                Ok((name, format!(" {}", path_and_query)))
            },
            ComponentIdentifier::Path => {
                let path = req.url().path();
                Ok((name, format!(" {}", path)))
            },
            ComponentIdentifier::Query => {
                let query = req.url().query().unwrap_or("");
                Ok((name, format!(" {}", query)))
            },
            ComponentIdentifier::Status => {
                // Not available for requests
                Err(SignatureError::InvalidParameter("@status not available for requests".to_string()))
            },
            ComponentIdentifier::Digest => {
                if let Some(digest) = req.headers().get("digest") {
                    let digest_str = digest.to_str()
                        .map_err(|_| SignatureError::InvalidHeader("Non-ASCII value in Digest header".to_string()))?;
                    Ok((name, format!(" {}", digest_str)))
                } else {
                    Err(SignatureError::InvalidHeader("Digest header not found".to_string()))
                }
            },
        }
    }
    
    /// Create a signature for a request using the given configuration
    pub fn sign_request(
        req: &mut Request,
        config: &SignatureConfig,
    ) -> Result<(), SignatureError> {
        // Create signature parameters
        let mut params = SignatureParameters::new();
        params.key_id = Some(config.key_id.clone());
        params.algorithm = Some(config.algorithm.clone());
        
        // Create signature base
        let signature_base = Self::create_signature_base(req, &config.components, &params)?;
        
        // Create a secure random number generator
        let rng = ring::rand::SystemRandom::new();
        
        // Sign the base with the secure random number generator
        let signature = Self::create_signature(&signature_base, &config.algorithm, &config.private_key, &rng)?;
        
        // Format signature input header
        let mut signature_input = String::new();
        for component in &config.components {
            signature_input.push_str(&format!("\"{}\" ", component.canonical_name()));
        }
        signature_input.push_str(&format!(";{}", params.format_parameters()));
        
        // Add signature headers
        let sig_input_header = HeaderValue::from_str(&format!("sig1={}", signature_input))
            .map_err(|_| SignatureError::InvalidHeader("Invalid signature-input header".to_string()))?;
            
        req.headers_mut().insert(
            HeaderName::from_static("signature-input"),
            sig_input_header,
        );
        
        let signature_header = HeaderValue::from_str(&format!("sig1=:{}", signature))
            .map_err(|_| SignatureError::InvalidHeader("Invalid signature header".to_string()))?;
            
        req.headers_mut().insert(
            HeaderName::from_static("signature"),
            signature_header,
        );
        
        Ok(())
    }
    
    /// Create the actual signature using the specified algorithm and private key
    fn create_signature(
        signature_base: &str,
        algorithm: &SignatureAlgorithm,
        private_key: &[u8],
        rng: &dyn ring::rand::SecureRandom,
    ) -> Result<String, SignatureError> {
        let signature = match algorithm {
            SignatureAlgorithm::Ed25519 => {
                let key_pair = Ed25519KeyPair::from_pkcs8(private_key)
                    .map_err(|e| SignatureError::InvalidKeyFormat(format!("Invalid Ed25519 key: {:?}", e)))?;
                
                let signature = key_pair.sign(signature_base.as_bytes());
                signature.as_ref().to_vec()
            },
            SignatureAlgorithm::EcdsaP256Sha256 => {
                let key_pair = EcdsaKeyPair::from_pkcs8(
                    &signature::ECDSA_P256_SHA256_ASN1_SIGNING,
                    private_key,
                    rng,
                ).map_err(|e| SignatureError::InvalidKeyFormat(format!("Invalid ECDSA key: {:?}", e)))?;
                
                let signature = key_pair.sign(rng, signature_base.as_bytes())
                    .map_err(|e| SignatureError::CryptoError(format!("Signing failed: {:?}", e)))?;
                
                signature.as_ref().to_vec()
            },
            SignatureAlgorithm::RsaSha256 => {
                let key_pair = RsaKeyPair::from_pkcs8(private_key)
                    .map_err(|e| SignatureError::InvalidKeyFormat(format!("Invalid RSA key: {:?}", e)))?;
                
                let mut signature = vec![0; key_pair.public_modulus_len()];
                key_pair.sign(
                    &signature::RSA_PKCS1_SHA256,
                    rng,
                    signature_base.as_bytes(),
                    &mut signature,
                ).map_err(|e| SignatureError::CryptoError(format!("RSA signing failed: {:?}", e)))?;
                
                signature
            },
            SignatureAlgorithm::RsaPssSha512 => {
                let key_pair = RsaKeyPair::from_pkcs8(private_key)
                    .map_err(|e| SignatureError::InvalidKeyFormat(format!("Invalid RSA key: {:?}", e)))?;
                
                let mut signature = vec![0; key_pair.public_modulus_len()];
                key_pair.sign(
                    &signature::RSA_PSS_SHA512,
                    rng,
                    signature_base.as_bytes(),
                    &mut signature,
                ).map_err(|e| SignatureError::CryptoError(format!("RSA-PSS signing failed: {:?}", e)))?;
                
                signature
            },
        };
        
        Ok(BASE64.encode(signature))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Client;

    #[test]
    fn test_signature_parameters_formatting() {
        let mut params = SignatureParameters::new();
        params.created = Some(DateTime::from_timestamp(1618884475, 0).unwrap());
        params.expires = Some(DateTime::from_timestamp(1618884775, 0).unwrap());
        params.key_id = Some("test-key-rsa-pss".to_string());
        params.algorithm = Some(SignatureAlgorithm::RsaPssSha512);
        
        let formatted = params.format_parameters();
        assert!(formatted.contains("created=1618884475"));
        assert!(formatted.contains("expires=1618884775"));
        assert!(formatted.contains("keyid=\"test-key-rsa-pss\""));
        assert!(formatted.contains("alg=\"rsa-pss-sha512\""));
    }
    
    #[test]
    fn test_component_identifier_from_str() {
        assert_eq!(
            ComponentIdentifier::from_str("@method").unwrap(),
            ComponentIdentifier::Method
        );
        
        assert_eq!(
            ComponentIdentifier::from_str("content-type").unwrap(),
            ComponentIdentifier::Header("content-type".to_string())
        );
        
        assert!(ComponentIdentifier::from_str("@invalid").is_err());
    }
    
    #[test]
    fn test_create_signature_base() {
        let client = Client::new();
        let mut req = client.post("https://example.com/foo?param=value")
            .header("host", "example.com")
            .header("content-type", "application/json")
            .header("digest", "sha-256=X48E9qOokqqrvdts8nOJRJN3OWDUoyWxBf7kbu9DBPE=")
            .header("content-length", "18")
            .build()
            .unwrap();
        
        let mut params = SignatureParameters::new();
        params.created = Some(DateTime::from_timestamp(1618884475, 0).unwrap());
        params.key_id = Some("test-key-ed25519".to_string());
        
        let components = vec![
            ComponentIdentifier::Method,
            ComponentIdentifier::Path,
            ComponentIdentifier::Header("content-type".to_string()),
            ComponentIdentifier::Header("digest".to_string()),
        ];
        
        let base = HttpSignature::create_signature_base(&req, &components, &params).unwrap();
        
        assert!(base.contains("\"@method\": POST"));
        assert!(base.contains("\"@path\": /foo"));
        assert!(base.contains("\"content-type\": application/json"));
        assert!(base.contains("\"digest\": sha-256=X48E9qOokqqrvdts8nOJRJN3OWDUoyWxBf7kbu9DBPE="));
        assert!(base.contains("\"@signature-params\":created=1618884475"));
    }
}
