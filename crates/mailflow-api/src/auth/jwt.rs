/// JWT validation using JWKS
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// JWT Claims structure based on the PRD specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Email address of the user
    pub email: String,

    /// Display name
    pub name: String,

    /// Subject (user ID)
    pub sub: String,

    /// Teams the user belongs to
    pub teams: Vec<String>,

    /// Resources the user has access to
    pub resources: Vec<Resource>,

    /// Audience
    pub aud: String,

    /// Expiration time (Unix timestamp)
    pub exp: usize,

    /// Issued at time (Unix timestamp)
    pub iat: usize,

    /// Issuer
    pub iss: String,
}

/// Resource access definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub host: String,
    pub method: String,
    pub path: String,
}

/// JWKS Key structure
#[derive(Debug, Deserialize)]
pub struct JwksKey {
    pub kty: String,
    pub kid: String,
    pub n: String,
    pub e: String,
}

/// JWKS structure
#[derive(Debug, Deserialize)]
pub struct Jwks {
    pub keys: Vec<JwksKey>,
}

/// JWT Validator
pub struct JwtValidator {
    /// JWKS keys mapped by kid
    keys: HashMap<String, DecodingKey>,
}

impl JwtValidator {
    /// Create a new JWT validator from JWKS JSON
    pub fn new(jwks_json: &str) -> Result<Self, String> {
        let jwks: Jwks =
            serde_json::from_str(jwks_json).map_err(|e| format!("Invalid JWKS JSON: {}", e))?;

        let mut keys = HashMap::new();

        for key in jwks.keys {
            if key.kty == "RSA" {
                let decoding_key = DecodingKey::from_rsa_components(&key.n, &key.e)
                    .map_err(|e| format!("Failed to create decoding key: {}", e))?;

                keys.insert(key.kid.clone(), decoding_key);
            }
        }

        if keys.is_empty() {
            return Err("No valid RSA keys found in JWKS".to_string());
        }

        Ok(Self { keys })
    }

    /// Validate a JWT token and return the claims
    pub fn validate(&self, token: &str, _expected_issuer: &str) -> Result<Claims, String> {
        // Decode header to get kid
        let header =
            decode_header(token).map_err(|e| format!("Failed to decode JWT header: {}", e))?;

        let kid = header
            .kid
            .ok_or_else(|| "JWT header missing 'kid' field".to_string())?;

        // Find matching key
        let decoding_key = self
            .keys
            .get(&kid)
            .ok_or_else(|| format!("No JWKS key found for kid: {}", kid))?;

        // Setup validation
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;
        // SKIP issuer validation - accept any issuer as long as token can be decoded with JWKS
        validation.validate_aud = false; // PRD says aud doesn't matter

        // Decode and validate token
        let token_data = decode::<Claims>(token, decoding_key, &validation)
            .map_err(|e| format!("Failed to validate JWT: {}", e))?;

        let claims = token_data.claims;

        // SKIP team validation - accept any valid JWT with valid signature
        // SKIP issuer validation - accept any issuer as long as token can be decoded with JWKS
        // Only validate: signature (JWKS), expiration, and token structure

        Ok(claims)
    }

    /// Extract JWT token from Authorization header
    pub fn extract_token(auth_header: Option<&str>) -> Result<String, String> {
        let auth_header = auth_header.ok_or_else(|| "Missing Authorization header".to_string())?;

        if !auth_header.starts_with("Bearer ") {
            return Err("Authorization header must start with 'Bearer '".to_string());
        }

        Ok(auth_header[7..].to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_token() {
        let token = JwtValidator::extract_token(Some("Bearer abc123")).unwrap();
        assert_eq!(token, "abc123");

        let result = JwtValidator::extract_token(Some("abc123"));
        assert!(result.is_err());

        let result = JwtValidator::extract_token(None);
        assert!(result.is_err());
    }
}
