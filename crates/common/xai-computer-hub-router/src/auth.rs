//! Dev-grade principal extraction.
//!
//! Mirrors the local-auth-dev hub contract the in-tree probe relies on
//! (`workspace_server_probe.rs`): the bearer is parsed as a JWT and the
//! payload's `sub` claim becomes the user id — the signature is NOT
//! verified. Anything unparsable falls back to a static dev user so an
//! unauthenticated loopback setup still works end to end.

use axum::http::HeaderMap;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use xai_tool_protocol::UserId;

/// Fallback principal for missing/opaque credentials.
const DEV_USER: &str = "local-dev";

/// Resolve the connection's user from upgrade headers.
pub(crate) fn user_from_headers(headers: &HeaderMap) -> UserId {
    let sub = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .and_then(jwt_sub);
    match sub {
        Some(sub) => UserId::new(&sub).unwrap_or_else(|_| dev_user()),
        None => dev_user(),
    }
}

fn dev_user() -> UserId {
    UserId::new(DEV_USER).expect("static dev user id is valid")
}

/// Extract `sub` from an (unverified) JWT payload.
fn jwt_sub(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.as_bytes()).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    Some(value.get("sub")?.as_str()?.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header::AUTHORIZATION;

    fn headers_with_bearer(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
        headers
    }

    /// Same shape as the probe's `dev_bearer`: unsigned JWT, `sub` claim.
    fn dev_bearer(user_id: &str) -> String {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
        let payload = URL_SAFE_NO_PAD.encode(format!(r#"{{"sub":"{user_id}"}}"#).as_bytes());
        format!("{header}.{payload}.")
    }

    #[test]
    fn unsigned_jwt_sub_becomes_user() {
        let headers = headers_with_bearer(&dev_bearer("usr-42"));
        assert_eq!(user_from_headers(&headers).as_str(), "usr-42");
    }

    #[test]
    fn missing_or_opaque_credential_falls_back_to_dev_user() {
        assert_eq!(user_from_headers(&HeaderMap::new()).as_str(), DEV_USER);
        let headers = headers_with_bearer("not-a-jwt");
        assert_eq!(user_from_headers(&headers).as_str(), DEV_USER);
    }
}
