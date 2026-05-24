//! JWT token generation and verification.

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// JWT claims for CDF authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,                    // principal_id
    pub username: String,
    pub roles: Vec<String>,
    pub namespaces: Vec<String>,
    pub iat: i64,                       // issued at
    pub exp: i64,                       // expiration
    pub jti: String,                    // token ID
    pub iss: String,                    // issuer
    pub aud: String,                    // audience
    pub mfa_verified: bool,
}

/// Configuration for JWT signing.
#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub issuer: String,
    pub audience: String,
    pub token_ttl: Duration,
    pub refresh_ttl: Duration,
    pub algorithm: jsonwebtoken::Algorithm,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: "change-me-in-production".into(),
            issuer: "cdf".into(),
            audience: "cdf-api".into(),
            token_ttl: Duration::hours(1),
            refresh_ttl: Duration::days(7),
            algorithm: jsonwebtoken::Algorithm::HS256,
        }
    }
}

/// Verifies and decodes JWT tokens.
pub struct JwtVerifier {
    config: JwtConfig,
}

impl JwtVerifier {
    pub fn new(config: JwtConfig) -> Self {
        Self { config }
    }

    pub fn generate(&self, claims: &Claims) -> Result<String, jsonwebtoken::errors::Error> {
        let encoding_key = EncodingKey::from_secret(self.config.secret.as_bytes());
        encode(&Header::new(self.config.algorithm), claims, &encoding_key)
    }

    pub fn verify(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let decoding_key = DecodingKey::from_secret(self.config.secret.as_bytes());
        let mut validation = Validation::new(self.config.algorithm);
        validation.set_issuer(&[&self.config.issuer]);
        validation.set_audience(&[&self.config.audience]);
        let token_data = decode::<Claims>(token, &decoding_key, &validation)?;
        Ok(token_data.claims)
    }

    pub fn refresh(&self, token: &str) -> Result<String, jsonwebtoken::errors::Error> {
        let claims = self.verify(token)?;
        let now = Utc::now();
        let new_claims = Claims {
            iat: now.timestamp(),
            exp: (now + self.config.token_ttl).timestamp(),
            jti: uuid::Uuid::new_v4().to_string(),
            ..claims
        };
        self.generate(&new_claims)
    }

    pub fn is_expired(&self, claims: &Claims) -> bool {
        Utc::now().timestamp() > claims.exp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_roundtrip() {
        let config = JwtConfig::default();
        let verifier = JwtVerifier::new(config);
        let claims = Claims {
            sub: "user1".into(),
            username: "alice".into(),
            roles: vec!["developer".into()],
            namespaces: vec!["default".into()],
            iat: Utc::now().timestamp(),
            exp: (Utc::now() + Duration::hours(1)).timestamp(),
            jti: uuid::Uuid::new_v4().to_string(),
            iss: "cdf".into(),
            aud: "cdf-api".into(),
            mfa_verified: false,
        };

        let token = verifier.generate(&claims).unwrap();
        let decoded = verifier.verify(&token).unwrap();
        assert_eq!(decoded.sub, "user1");
        assert_eq!(decoded.username, "alice");
    }

    #[test]
    fn test_expired_token() {
        let config = JwtConfig::default();
        let verifier = JwtVerifier::new(config);
        let claims = Claims {
            sub: "user1".into(),
            username: "alice".into(),
            roles: vec![],
            namespaces: vec![],
            iat: (Utc::now() - Duration::hours(2)).timestamp(),
            exp: (Utc::now() - Duration::hours(1)).timestamp(),
            jti: uuid::Uuid::new_v4().to_string(),
            iss: "cdf".into(),
            aud: "cdf-api".into(),
            mfa_verified: false,
        };

        let token = verifier.generate(&claims).unwrap();
        assert!(verifier.verify(&token).is_err());
    }
}
