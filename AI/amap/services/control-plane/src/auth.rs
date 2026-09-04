//! OIDC bearer-token verification for the control plane (stack §17).
//!
//! HITL approvals are the one place where a human overrides the deterministic gate, so the
//! approver's identity must be verified against an identity provider rather than self-declared
//! in a request header. Tokens are validated offline against the issuer's JWKS (signature,
//! `iss`, `aud`, `exp`, `nbf`), and reviewer roles are read from a configurable claim.
use jsonwebtoken::jwk::{AlgorithmParameters, Jwk, JwkSet};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct OidcConfig {
    pub issuer: String,
    pub audience: Option<String>,
    pub jwks_url: Option<String>,
    /// Dotted path of the roles claim, e.g. `roles` or `realm_access.roles`.
    pub roles_claim: String,
}

/// A verified human identity.
#[derive(Clone, Debug, PartialEq)]
pub struct Identity {
    /// Stable subject identifier (`sub`).
    pub subject: String,
    pub issuer: String,
    /// Human-readable name when the token carries one.
    pub display: Option<String>,
    pub roles: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("malformed token: {0}")]
    Malformed(String),
    #[error("no signing key matches the token")]
    UnknownKey,
    #[error("token rejected: {0}")]
    Invalid(String),
    #[error("identity provider unreachable: {0}")]
    Provider(String),
}

struct CachedKeys {
    set: JwkSet,
    fetched_at: Instant,
}

pub struct OidcVerifier {
    config: OidcConfig,
    http: reqwest::Client,
    jwks_url: Option<String>,
    keys: RwLock<CachedKeys>,
    refresh_min_interval: Duration,
    key_ttl: Duration,
}

#[derive(Deserialize)]
struct Discovery {
    jwks_uri: String,
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
    #[serde(default)]
    preferred_username: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(flatten)]
    rest: serde_json::Map<String, Value>,
}

fn allowed_algorithms(jwk: &Jwk) -> Vec<Algorithm> {
    if let Some(alg) = jwk.common.key_algorithm {
        return Algorithm::try_from(alg)
            .map(|a| vec![a])
            .unwrap_or_default();
    }
    match &jwk.algorithm {
        AlgorithmParameters::RSA(_) => vec![
            Algorithm::RS256,
            Algorithm::RS384,
            Algorithm::RS512,
            Algorithm::PS256,
            Algorithm::PS384,
            Algorithm::PS512,
        ],
        AlgorithmParameters::EllipticCurve(_) => vec![Algorithm::ES256, Algorithm::ES384],
        AlgorithmParameters::OctetKeyPair(_) => vec![Algorithm::EdDSA],
        AlgorithmParameters::OctetKey(_) => {
            vec![Algorithm::HS256, Algorithm::HS384, Algorithm::HS512]
        }
        // `Other` and any key type added by the library later: never guess an algorithm.
        _ => vec![],
    }
}

fn claim_path<'a>(root: &'a serde_json::Map<String, Value>, path: &str) -> Option<&'a Value> {
    let mut segments = path.split('.').filter(|s| !s.is_empty());
    let first = segments.next()?;
    let mut current = root.get(first)?;
    for segment in segments {
        current = current.get(segment)?;
    }
    Some(current)
}

fn roles_from(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect(),
        Some(Value::String(text)) => text.split_whitespace().map(str::to_owned).collect(),
        _ => vec![],
    }
}

impl OidcVerifier {
    /// Resolve the JWKS endpoint (via OpenID discovery unless configured) and load the keys.
    pub async fn discover(config: OidcConfig) -> Result<Self, AuthError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| AuthError::Provider(e.to_string()))?;
        let jwks_url = match &config.jwks_url {
            Some(url) => url.clone(),
            None => {
                let url = format!(
                    "{}/.well-known/openid-configuration",
                    config.issuer.trim_end_matches('/')
                );
                http.get(&url)
                    .send()
                    .await
                    .and_then(|r| r.error_for_status())
                    .map_err(|e| AuthError::Provider(format!("{url}: {e}")))?
                    .json::<Discovery>()
                    .await
                    .map_err(|e| AuthError::Provider(format!("{url}: {e}")))?
                    .jwks_uri
            }
        };
        let set = Self::fetch(&http, &jwks_url).await?;
        Ok(Self {
            config,
            http,
            jwks_url: Some(jwks_url),
            keys: RwLock::new(CachedKeys {
                set,
                fetched_at: Instant::now(),
            }),
            refresh_min_interval: Duration::from_secs(60),
            key_ttl: Duration::from_secs(3600),
        })
    }

    /// Verifier over a fixed key set (tests, air-gapped deployments with pinned keys).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_static_keys(config: OidcConfig, set: JwkSet) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
            jwks_url: None,
            keys: RwLock::new(CachedKeys {
                set,
                fetched_at: Instant::now(),
            }),
            refresh_min_interval: Duration::from_secs(60),
            key_ttl: Duration::MAX,
        }
    }

    async fn fetch(http: &reqwest::Client, url: &str) -> Result<JwkSet, AuthError> {
        http.get(url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| AuthError::Provider(format!("{url}: {e}")))?
            .json::<JwkSet>()
            .await
            .map_err(|e| AuthError::Provider(format!("{url}: {e}")))
    }

    async fn find_key(&self, kid: Option<&str>) -> Option<Jwk> {
        let keys = self.keys.read().await;
        match kid {
            Some(kid) => keys.set.find(kid).cloned(),
            // Without a `kid` only an unambiguous single-key set is acceptable.
            None if keys.set.keys.len() == 1 => keys.set.keys.first().cloned(),
            None => None,
        }
    }

    /// Refresh the JWKS if a refresh is allowed (rate-limited). Returns whether keys changed.
    async fn refresh(&self) -> Result<bool, AuthError> {
        let Some(url) = &self.jwks_url else {
            return Ok(false);
        };
        {
            let keys = self.keys.read().await;
            if keys.fetched_at.elapsed() < self.refresh_min_interval {
                return Ok(false);
            }
        }
        let fresh = Self::fetch(&self.http, url).await?;
        let mut keys = self.keys.write().await;
        let changed = fresh != keys.set;
        keys.set = fresh;
        keys.fetched_at = Instant::now();
        Ok(changed)
    }

    pub async fn verify(&self, token: &str) -> Result<Identity, AuthError> {
        let header = decode_header(token).map_err(|e| AuthError::Malformed(e.to_string()))?;
        let kid = header.kid.as_deref();
        let stale = self.keys.read().await.fetched_at.elapsed() > self.key_ttl;
        let mut jwk = self.find_key(kid).await;
        if jwk.is_none() || stale {
            self.refresh().await?;
            jwk = self.find_key(kid).await;
        }
        let jwk = jwk.ok_or(AuthError::UnknownKey)?;
        let algorithms = allowed_algorithms(&jwk);
        if !algorithms.contains(&header.alg) {
            return Err(AuthError::Invalid(format!(
                "algorithm {:?} not permitted for the signing key",
                header.alg
            )));
        }
        let key = DecodingKey::from_jwk(&jwk).map_err(|e| AuthError::Invalid(e.to_string()))?;
        let mut validation = Validation::new(header.alg);
        validation.algorithms = algorithms;
        validation.leeway = 30;
        validation.set_issuer(&[self.config.issuer.as_str()]);
        match &self.config.audience {
            Some(aud) => validation.set_audience(&[aud.as_str()]),
            None => validation.validate_aud = false,
        }
        validation.set_required_spec_claims(&["exp", "iss", "sub"]);
        let data = decode::<Claims>(token, &key, &validation)
            .map_err(|e| AuthError::Invalid(e.to_string()))?;
        let claims = data.claims;
        if claims.sub.trim().is_empty() {
            return Err(AuthError::Invalid("empty subject".into()));
        }
        let roles = roles_from(claim_path(&claims.rest, &self.config.roles_claim));
        Ok(Identity {
            subject: claims.sub,
            issuer: self.config.issuer.clone(),
            display: claims
                .preferred_username
                .or(claims.email)
                .or(claims.name)
                .filter(|s| !s.trim().is_empty()),
            roles,
        })
    }
}

#[cfg(test)]
pub mod testing {
    //! Helpers shared with the control-plane integration tests: an HS256 key set and a signer.
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde_json::json;

    pub const ISSUER: &str = "https://idp.example.test/realms/amap";
    pub const AUDIENCE: &str = "amap-control-plane";
    pub const KID: &str = "test-key-1";
    const SECRET: &[u8] = b"secret-secret-secret-secret-secret";
    // base64url(SECRET) as it would appear in a JWKS `oct` entry
    const SECRET_B64URL: &str = "c2VjcmV0LXNlY3JldC1zZWNyZXQtc2VjcmV0LXNlY3JldA";

    pub fn config() -> OidcConfig {
        OidcConfig {
            issuer: ISSUER.into(),
            audience: Some(AUDIENCE.into()),
            jwks_url: None,
            roles_claim: "realm_access.roles".into(),
        }
    }

    pub fn jwks() -> JwkSet {
        serde_json::from_value(json!({
            "keys": [{ "kty": "oct", "kid": KID, "alg": "HS256", "k": SECRET_B64URL }]
        }))
        .unwrap()
    }

    pub fn now() -> u64 {
        jsonwebtoken::get_current_timestamp()
    }

    /// Sign a token with the test key; `overrides` are merged into the default claims.
    pub fn token(overrides: Value) -> String {
        let mut claims = json!({
            "iss": ISSUER,
            "aud": AUDIENCE,
            "sub": "user-42",
            "preferred_username": "alice",
            "exp": now() + 600,
            "iat": now(),
            "realm_access": { "roles": ["sme", "viewer"] }
        });
        if let (Value::Object(base), Value::Object(extra)) = (&mut claims, overrides) {
            for (k, v) in extra {
                base.insert(k, v);
            }
        }
        let mut header = Header::new(Algorithm::HS256);
        header.kid = Some(KID.into());
        encode(&header, &claims, &EncodingKey::from_secret(SECRET)).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use serde_json::json;

    fn verifier() -> OidcVerifier {
        OidcVerifier::with_static_keys(config(), jwks())
    }

    #[tokio::test]
    async fn accepts_valid_token_and_extracts_roles() {
        let id = verifier().verify(&token(json!({}))).await.unwrap();
        assert_eq!(id.subject, "user-42");
        assert_eq!(id.display.as_deref(), Some("alice"));
        assert_eq!(id.roles, vec!["sme".to_string(), "viewer".to_string()]);
        assert_eq!(id.issuer, ISSUER);
    }

    #[tokio::test]
    async fn rejects_expired_wrong_audience_wrong_issuer_and_tampering() {
        let v = verifier();
        for (label, claims) in [
            ("expired", json!({ "exp": now() - 120 })),
            ("audience", json!({ "aud": "someone-else" })),
            ("issuer", json!({ "iss": "https://evil.example" })),
        ] {
            let err = v.verify(&token(claims)).await.unwrap_err();
            assert!(matches!(err, AuthError::Invalid(_)), "{label}: {err}");
        }
        let mut tampered = token(json!({}));
        tampered.push('x');
        assert!(v.verify(&tampered).await.is_err());
        assert!(matches!(
            v.verify("not-a-jwt").await.unwrap_err(),
            AuthError::Malformed(_)
        ));
    }

    #[tokio::test]
    async fn unknown_key_id_is_rejected_without_a_refresh_source() {
        let v = verifier();
        let mut header = jsonwebtoken::Header::new(Algorithm::HS256);
        header.kid = Some("other-key".into());
        let signed = jsonwebtoken::encode(
            &header,
            &json!({ "iss": ISSUER, "aud": AUDIENCE, "sub": "x", "exp": now() + 60 }),
            &jsonwebtoken::EncodingKey::from_secret(b"whatever"),
        )
        .unwrap();
        assert!(matches!(
            v.verify(&signed).await.unwrap_err(),
            AuthError::UnknownKey
        ));
    }

    #[tokio::test]
    async fn roles_claim_accepts_space_separated_scopes_and_missing_claims() {
        let cfg = OidcConfig {
            roles_claim: "scope".into(),
            ..config()
        };
        let v = OidcVerifier::with_static_keys(cfg, jwks());
        let id = v
            .verify(&token(json!({ "scope": "openid amap:sme" })))
            .await
            .unwrap();
        assert_eq!(id.roles, vec!["openid".to_string(), "amap:sme".to_string()]);
        let none = v.verify(&token(json!({}))).await.unwrap();
        assert!(none.roles.is_empty());
    }
}
