//! Linear GraphQL HTTP 클라이언트: 요청 전송, 오류 분류, 한도 추적.

use std::sync::Mutex;
use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

pub const LINEAR_ENDPOINT: &str = "https://api.linear.app/graphql";

/// 남은 요청이 이보다 적으면 자동 요청(주기 새로고침·서버 검색)을 멈춘다.
pub const AUTO_PAUSE_THRESHOLD: i64 = 50;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    #[error("Linear API 한도를 넘었어요")]
    RateLimited { reset_at_ms: Option<i64> },
    #[error("API 키가 만료됐거나 권한이 없어요")]
    Auth,
    #[error("Linear 오류: {0}")]
    GraphQl(String),
    #[error("오프라인: {0}")]
    Offline(String),
    #[error("응답을 해석하지 못했어요: {0}")]
    Decode(String),
}

/// 마지막 응답의 한도 헤더.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RateLimit {
    pub requests_remaining: Option<i64>,
    /// UTC epoch 밀리초
    pub requests_reset_ms: Option<i64>,
    pub complexity: Option<i64>,
}

impl RateLimit {
    /// 자동 요청을 멈춰야 하는지. 리셋 시각이 지나면 다시 허용한다.
    pub fn should_pause_auto(&self, now_ms: i64) -> bool {
        match self.requests_remaining {
            Some(r) if r < AUTO_PAUSE_THRESHOLD => {
                self.requests_reset_ms.is_none_or(|reset| now_ms < reset)
            }
            _ => false,
        }
    }
}

pub struct LinearClient {
    agent: ureq::Agent,
    endpoint: String,
    api_key: String,
    rate: Mutex<RateLimit>,
}

impl LinearClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        LinearClient::with_endpoint(api_key, LINEAR_ENDPOINT)
    }

    /// 테스트에서 가짜 서버 주소를 넣을 때 쓴다.
    pub fn with_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(5)))
            .timeout_global(Some(Duration::from_secs(15)))
            .http_status_as_error(false)
            .build()
            .into();
        LinearClient {
            agent,
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            rate: Mutex::new(RateLimit::default()),
        }
    }

    pub fn rate_limit(&self) -> RateLimit {
        *self.rate.lock().unwrap()
    }

    /// 키 지문. 캐시가 어느 키로 받은 것인지 구분할 때 쓴다. 키 자체는 드러나지 않는다.
    pub fn key_fingerprint(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.api_key.hash(&mut h);
        format!("{:016x}", h.finish())
    }

    /// GraphQL 요청을 보내고 응답의 `data`를 `T`로 돌려준다.
    pub fn execute<T: DeserializeOwned>(
        &self,
        query: &str,
        variables: Value,
    ) -> Result<T, ApiError> {
        let body = json!({ "query": query, "variables": variables });
        let mut resp = self
            .agent
            .post(&self.endpoint)
            .header("Authorization", self.api_key.as_str())
            .send_json(&body)
            .map_err(|e| ApiError::Offline(e.to_string()))?;
        let status = resp.status().as_u16();
        let rate = read_rate_limit(resp.headers());
        if rate.requests_remaining.is_some() || rate.complexity.is_some() {
            *self.rate.lock().unwrap() = rate;
        }
        let text = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| ApiError::Offline(e.to_string()))?;
        parse_response(status, &text, rate.requests_reset_ms)
    }
}

fn read_rate_limit(headers: &ureq::http::HeaderMap) -> RateLimit {
    let num = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<i64>().ok())
    };
    RateLimit {
        requests_remaining: num("x-ratelimit-requests-remaining"),
        requests_reset_ms: num("x-ratelimit-requests-reset"),
        complexity: num("x-complexity"),
    }
}

#[derive(Deserialize)]
struct GqlResponse {
    data: Option<Value>,
    #[serde(default)]
    errors: Vec<GqlError>,
}

#[derive(Deserialize)]
struct GqlError {
    #[serde(default)]
    message: String,
    #[serde(default)]
    extensions: Option<GqlExtensions>,
}

#[derive(Deserialize)]
struct GqlExtensions {
    code: Option<String>,
}

/// 응답을 해석한다. Linear는 한도 초과를 HTTP 400 + `RATELIMITED` 코드로 알린다.
fn parse_response<T: DeserializeOwned>(
    status: u16,
    text: &str,
    reset_ms: Option<i64>,
) -> Result<T, ApiError> {
    let resp: GqlResponse = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(e) => {
            return Err(match status {
                401 | 403 => ApiError::Auth,
                429 => ApiError::RateLimited {
                    reset_at_ms: reset_ms,
                },
                s if s >= 500 => ApiError::Offline(format!("Linear 서버 오류 ({s})")),
                _ => ApiError::Decode(e.to_string()),
            });
        }
    };
    if !resp.errors.is_empty() {
        let has_code = |code: &str| {
            resp.errors
                .iter()
                .any(|e| e.extensions.as_ref().and_then(|x| x.code.as_deref()) == Some(code))
        };
        if has_code("RATELIMITED") {
            return Err(ApiError::RateLimited {
                reset_at_ms: reset_ms,
            });
        }
        if has_code("AUTHENTICATION_ERROR") || status == 401 {
            return Err(ApiError::Auth);
        }
        return Err(ApiError::GraphQl(resp.errors[0].message.clone()));
    }
    if status >= 500 {
        return Err(ApiError::Offline(format!("Linear 서버 오류 ({status})")));
    }
    if status == 401 || status == 403 {
        return Err(ApiError::Auth);
    }
    let data = resp
        .data
        .ok_or_else(|| ApiError::Decode("data가 없어요".to_string()))?;
    serde_json::from_value(data).map_err(|e| ApiError::Decode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Matcher;

    #[derive(Debug, Deserialize, PartialEq)]
    struct ViewerData {
        viewer: ViewerId,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct ViewerId {
        id: String,
    }

    fn client(server: &mockito::Server) -> LinearClient {
        LinearClient::with_endpoint("lin_api_test", format!("{}/graphql", server.url()))
    }

    #[test]
    fn sends_raw_key_and_returns_data() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("POST", "/graphql")
            .match_header("authorization", "lin_api_test")
            .match_header("content-type", Matcher::Regex("application/json".into()))
            .match_body(Matcher::PartialJson(
                json!({ "query": "{ viewer { id } }" }),
            ))
            .with_status(200)
            .with_header("x-ratelimit-requests-remaining", "2499")
            .with_header("x-ratelimit-requests-reset", "1791290000000")
            .with_header("x-complexity", "12")
            .with_body(r#"{"data":{"viewer":{"id":"u1"}}}"#)
            .create();
        let c = client(&server);
        let got: ViewerData = c.execute("{ viewer { id } }", json!({})).unwrap();
        assert_eq!(got.viewer.id, "u1");
        assert_eq!(
            c.rate_limit(),
            RateLimit {
                requests_remaining: Some(2499),
                requests_reset_ms: Some(1_791_290_000_000),
                complexity: Some(12),
            }
        );
        m.assert();
    }

    #[test]
    fn rate_limited_is_http_400_with_code() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_header("x-ratelimit-requests-remaining", "0")
            .with_header("x-ratelimit-requests-reset", "1791290000000")
            .with_body(r#"{"errors":[{"message":"Rate limit exceeded","extensions":{"code":"RATELIMITED"}}]}"#)
            .create();
        let err = client(&server)
            .execute::<Value>("{ x }", json!({}))
            .unwrap_err();
        assert_eq!(
            err,
            ApiError::RateLimited {
                reset_at_ms: Some(1_791_290_000_000)
            }
        );
    }

    #[test]
    fn authentication_error_code_means_auth() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_body(r#"{"errors":[{"message":"Authentication required","extensions":{"code":"AUTHENTICATION_ERROR"}}]}"#)
            .create();
        let err = client(&server)
            .execute::<Value>("{ x }", json!({}))
            .unwrap_err();
        assert_eq!(err, ApiError::Auth);
    }

    #[test]
    fn plain_401_means_auth() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(401)
            .with_body("Unauthorized")
            .create();
        let err = client(&server)
            .execute::<Value>("{ x }", json!({}))
            .unwrap_err();
        assert_eq!(err, ApiError::Auth);
    }

    #[test]
    fn other_graphql_error_keeps_message() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(400)
            .with_body(r#"{"errors":[{"message":"Field 'x' doesn't exist","extensions":{"code":"GRAPHQL_VALIDATION_FAILED"}}]}"#)
            .create();
        let err = client(&server)
            .execute::<Value>("{ x }", json!({}))
            .unwrap_err();
        assert_eq!(err, ApiError::GraphQl("Field 'x' doesn't exist".into()));
    }

    #[test]
    fn server_error_is_offline() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(503)
            .with_body("busy")
            .create();
        let err = client(&server)
            .execute::<Value>("{ x }", json!({}))
            .unwrap_err();
        assert!(matches!(err, ApiError::Offline(_)), "{err:?}");
    }

    #[test]
    fn unreachable_server_is_offline() {
        let c = LinearClient::with_endpoint("k", "http://127.0.0.1:9/graphql");
        let err = c.execute::<Value>("{ x }", json!({})).unwrap_err();
        assert!(matches!(err, ApiError::Offline(_)), "{err:?}");
    }

    #[test]
    fn invalid_json_is_decode_error() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/graphql")
            .with_status(200)
            .with_body("<html>")
            .create();
        let err = client(&server)
            .execute::<Value>("{ x }", json!({}))
            .unwrap_err();
        assert!(matches!(err, ApiError::Decode(_)), "{err:?}");
    }

    #[test]
    fn pause_only_when_low_and_before_reset() {
        let low = RateLimit {
            requests_remaining: Some(10),
            requests_reset_ms: Some(2_000),
            complexity: None,
        };
        assert!(low.should_pause_auto(1_000));
        assert!(!low.should_pause_auto(3_000));
        let plenty = RateLimit {
            requests_remaining: Some(500),
            requests_reset_ms: Some(2_000),
            complexity: None,
        };
        assert!(!plenty.should_pause_auto(1_000));
        assert!(!RateLimit::default().should_pause_auto(1_000));
    }

    #[test]
    fn fingerprint_is_stable_per_key_and_hides_it() {
        let a = LinearClient::new("lin_api_aaaa");
        let a2 = LinearClient::new("lin_api_aaaa");
        let b = LinearClient::new("lin_api_bbbb");
        assert_eq!(a.key_fingerprint(), a2.key_fingerprint());
        assert_ne!(a.key_fingerprint(), b.key_fingerprint());
        assert!(!a.key_fingerprint().contains("aaaa"));
    }
}
