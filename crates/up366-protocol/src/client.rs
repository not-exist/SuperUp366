//! Authenticated HTTP transport.
//!
//! The client owns three pieces of state the rest of the crate relies on:
//!
//! 1. the seven fixed headers the official PC client sends on every request;
//! 2. a cookie jar that survives login and can be exported into
//!    [`Session::cookies`] for persistence;
//! 3. the TGT used as `u3t` and for one-time service-ticket exchange.
//!
//! Requests are executed through [`Endpoint`] entries only. A failed business
//! call with code `-21`/`-22` is retried exactly once with a service ticket;
//! transient read failures retry at most twice with exponential backoff, while
//! writes never retry. Both limits are deliberate: replaying a write is a
//! correctness bug, not an optimization.

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::cookie::CookieStore;
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT,
};
use reqwest::Client as HttpClient;
use serde_json::Value;
use url::Url;
use uuid::Uuid;

use crate::codec;
use crate::endpoint::{get, BodyKind, Endpoint};
use crate::error::{ProtocolError, Result};
use crate::form::{encode_value, normalize_key};
use crate::session::Session;

/// Exact user agent sent by the official PC student client.
pub const USER_AGENT_VALUE: &str = "PC-Up366-Student 6.11.0";
/// Default request timeout, matching the reference client.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);
/// Maximum transient retries for a read request.
pub const MAX_READ_RETRIES: u32 = 2;
/// First retry delay; doubles per attempt.
pub const DEFAULT_RETRY_BASE: Duration = Duration::from_millis(200);

type ClockFn = dyn Fn() -> u128 + Send + Sync + 'static;

/// Builds a [`Client`].
///
/// ```
/// # use up366_protocol::Client;
/// let client = Client::builder()
///     .client_id("MY-DEVICE")
///     .base_url_override("http://127.0.0.1:8080") // tests / proxies only
///     .build()
///     .unwrap();
/// assert_eq!(client.client_id(), "MY-DEVICE");
/// ```
#[derive(Default)]
pub struct ClientBuilder {
    client_id: Option<String>,
    session: Option<Session>,
    clock: Option<Arc<ClockFn>>,
    base_override: Option<String>,
    retry_base_delay: Option<Duration>,
    max_read_retries: Option<u32>,
    timeout: Option<Duration>,
}

impl ClientBuilder {
    /// Use a fixed device id (the `clientId` header).
    pub fn client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self
    }

    /// Restore a previously persisted session.
    pub fn session(mut self, session: Session) -> Self {
        self.session = Some(session);
        self
    }

    /// Replace the millisecond clock used by request signatures.
    pub fn clock<F>(mut self, clock: F) -> Self
    where
        F: Fn() -> u128 + Send + Sync + 'static,
    {
        self.clock = Some(Arc::new(clock));
        self
    }

    /// Send every endpoint to `origin` instead of its production host.
    ///
    /// This is a test/proxy hook: endpoint paths are preserved, scheme+host are
    /// replaced. It is how the crate tests the transport without touching the
    /// real network.
    pub fn base_url_override(mut self, origin: impl Into<String>) -> Self {
        self.base_override = Some(origin.into().trim_end_matches('/').to_owned());
        self
    }

    /// Base delay for transient read retries (default 200 ms).
    pub fn retry_base_delay(mut self, delay: Duration) -> Self {
        self.retry_base_delay = Some(delay);
        self
    }

    /// Maximum transient read retries (default 2).
    pub fn max_read_retries(mut self, retries: u32) -> Self {
        self.max_read_retries = Some(retries);
        self
    }

    /// Per-request timeout (default 20 s).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Create the client.
    pub fn build(self) -> Result<Client> {
        let client_id: Arc<str> = Arc::from(
            self.client_id
                .unwrap_or_else(|| Uuid::new_v4().simple().to_string().to_uppercase()),
        );
        let import_domain = self
            .base_override
            .as_deref()
            .and_then(|origin| Url::parse(origin).ok())
            .and_then(|url| url.host_str().map(str::to_owned));

        let jar = Arc::new(CookieJar::new());
        let mut state = ClientState::default();
        if let Some(session) = &self.session {
            jar.replace(&session.cookies, import_domain.as_deref());
            state.username.clone_from(&session.username);
            state.tgt.clone_from(&session.tgt);
            state.user_type = session.user_type;
            state.expired = session.expired;
        }

        let http = HttpClient::builder()
            .cookie_provider(jar.clone())
            .timeout(self.timeout.unwrap_or(DEFAULT_TIMEOUT))
            .build()
            .map_err(|error| ProtocolError::Transport(error.to_string()))?;

        Ok(Client {
            http,
            cookies: jar,
            state: Arc::new(RwLock::new(state)),
            client_id,
            clock: self.clock.unwrap_or_else(|| Arc::new(system_now_ms)),
            base_override: self.base_override.map(Arc::from),
            retry_base_delay: self.retry_base_delay.unwrap_or(DEFAULT_RETRY_BASE),
            max_read_retries: self.max_read_retries.unwrap_or(MAX_READ_RETRIES),
        })
    }
}

#[derive(Debug, Clone, Default)]
struct ClientState {
    username: String,
    tgt: String,
    user_type: Option<i64>,
    expired: bool,
}

/// Authenticated (or anonymous, before login) HTTP client.
#[derive(Clone)]
pub struct Client {
    http: HttpClient,
    cookies: Arc<CookieJar>,
    state: Arc<RwLock<ClientState>>,
    client_id: Arc<str>,
    clock: Arc<ClockFn>,
    base_override: Option<Arc<str>>,
    retry_base_delay: Duration,
    max_read_retries: u32,
}

impl Client {
    /// Start a builder.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Anonymous client with a random device id.
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    /// Anonymous client with an explicit device id.
    pub fn with_client_id(client_id: impl Into<String>) -> Result<Self> {
        Self::builder().client_id(client_id).build()
    }

    /// Restore a client from a persisted session.
    pub fn with_session(session: Session) -> Result<Self> {
        Self::builder().session(session).build()
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Current millisecond timestamp used by request signatures.
    pub fn now_ms(&self) -> u128 {
        (self.clock)()
    }

    pub fn username(&self) -> String {
        self.read_state().username
    }

    pub fn tgt(&self) -> String {
        self.read_state().tgt
    }

    pub fn user_type(&self) -> Option<i64> {
        self.read_state().user_type
    }

    pub fn is_expired(&self) -> bool {
        self.read_state().expired
    }

    /// Mark the session unusable after an upstream rejection.
    pub fn mark_expired(&self) {
        self.write_state().expired = true;
    }

    /// Replace the TGT and clear the expired flag.
    pub fn set_tgt(&self, tgt: impl Into<String>) {
        let mut state = self.write_state();
        state.tgt = tgt.into();
        state.expired = false;
    }

    /// Replace all imported cookies with an explicit set.
    pub fn import_cookies(&self, cookies: &[(String, String)]) {
        let domain = self.import_domain();
        self.cookies.replace(cookies, domain.as_deref());
    }

    /// Export the current cookie names/values for persistence.
    pub fn export_cookies(&self) -> Vec<(String, String)> {
        self.cookies.export()
    }

    /// Snapshot the current session, including cookies.
    pub fn session(&self) -> Session {
        let state = self.read_state();
        Session {
            username: state.username,
            tgt: state.tgt,
            client_id: self.client_id.to_string(),
            user_type: state.user_type,
            cookies: self.export_cookies(),
            expired: state.expired,
        }
    }

    /// Apply a session (for example after a fresh login).
    pub fn apply_session(&self, session: &Session) {
        {
            let mut state = self.write_state();
            state.username.clone_from(&session.username);
            state.tgt.clone_from(&session.tgt);
            state.user_type = session.user_type;
            state.expired = session.expired;
        }
        let domain = self.import_domain();
        self.cookies.replace(&session.cookies, domain.as_deref());
    }

    /// Execute an [`Endpoint`] and return its `data` payload.
    ///
    /// `fields` are sent as a form body for [`BodyKind::Form`], as query
    /// parameters for [`BodyKind::Query`], and ignored for [`BodyKind::None`].
    pub async fn call(
        &self,
        endpoint: &Endpoint,
        fields: &[(&str, Value)],
    ) -> Result<Option<Value>> {
        self.execute(endpoint, fields, &[codec::OK_CODE], false)
            .await
    }

    /// Execute with custom accepted business codes. Used by login (`-26`) and
    /// service-ticket issuance (`-24`).
    pub(crate) async fn call_with_codes(
        &self,
        endpoint: &Endpoint,
        fields: &[(&str, Value)],
        ok_codes: &[i64],
        anonymous: bool,
    ) -> Result<Option<Value>> {
        self.execute(endpoint, fields, ok_codes, anonymous).await
    }

    async fn execute(
        &self,
        endpoint: &Endpoint,
        fields: &[(&str, Value)],
        ok_codes: &[i64],
        anonymous: bool,
    ) -> Result<Option<Value>> {
        let base = self.endpoint_url(endpoint);
        let url = if endpoint.body == BodyKind::Query {
            append_query(&base, fields)?
        } else {
            base
        };

        let mut attempt = 0u32;
        loop {
            let result = self
                .execute_once(endpoint, &url, fields, ok_codes, anonymous, !anonymous)
                .await;
            match result {
                Err(error)
                    if !endpoint.write
                        && attempt < self.max_read_retries
                        && error.is_retryable() =>
                {
                    let factor = 1u32.checked_shl(attempt).unwrap_or(u32::MAX);
                    let delay = self.retry_base_delay.saturating_mul(factor);
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    async fn execute_once(
        &self,
        endpoint: &Endpoint,
        url: &str,
        fields: &[(&str, Value)],
        ok_codes: &[i64],
        anonymous: bool,
        allow_ticket: bool,
    ) -> Result<Option<Value>> {
        let payload = self.send_raw(endpoint, url, fields, anonymous).await?;

        match codec::parse_envelope_with_codes(payload, ok_codes) {
            Ok(data) => Ok(data),
            Err(ProtocolError::Api { code, .. }) if allow_ticket && matches!(code, -21 | -22) => {
                let ticket = match self.service_ticket(url).await {
                    Ok(ticket) => ticket,
                    Err(error) => {
                        // Any business rejection here means the SSO host would
                        // not trade this TGT for a ticket. The exact code is
                        // not worth matching ("找不到tgt" comes back under more
                        // than one), so treat the session as dead.
                        if matches!(
                            error,
                            ProtocolError::Api { .. }
                                | ProtocolError::Http {
                                    status: 401 | 403,
                                    ..
                                }
                        ) {
                            self.mark_expired();
                        }
                        return Err(error);
                    }
                };
                let replay_url = append_query(url, &[("ticket", Value::String(ticket))])?;
                let result = match self
                    .send_raw(endpoint, &replay_url, fields, anonymous)
                    .await
                {
                    Ok(payload) => codec::parse_envelope_with_codes(payload, ok_codes),
                    Err(error) => Err(error),
                };
                if let Err(error) = &result {
                    self.mark_expired_if_needed(error);
                }
                result
            }
            Err(error) => {
                // A business host can answer "tgt not found" without going
                // through the ticket fallback (only -21/-22 trigger it).
                self.mark_expired_if_needed(&error);
                Err(error)
            }
        }
    }

    async fn send_raw(
        &self,
        endpoint: &Endpoint,
        url: &str,
        fields: &[(&str, Value)],
        anonymous: bool,
    ) -> Result<Value> {
        let mut request = match endpoint.body {
            BodyKind::Query => self.http.get(url),
            BodyKind::Form | BodyKind::None => self.http.post(url),
        };
        request = request.headers(self.headers(anonymous)?);
        if endpoint.body == BodyKind::Form {
            request = request
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(encode_form_body(fields));
        }

        let response = request.send().await.map_err(ProtocolError::from)?;
        let status = response.status();
        let body = response.text().await.map_err(ProtocolError::from)?;
        if !status.is_success() {
            return Err(ProtocolError::Http {
                status: status.as_u16(),
                body: truncate(&body, 300),
            });
        }

        serde_json::from_str(&body).map_err(|error| {
            ProtocolError::Decode(format!("{}: invalid JSON response: {error}", endpoint.name))
        })
    }

    /// Exchange the TGT for a one-time ticket for `service`.
    async fn service_ticket(&self, service: &str) -> Result<String> {
        let endpoint = get("service_ticket")
            .ok_or_else(|| ProtocolError::Decode("missing endpoint: service_ticket".into()))?;
        let url = self.endpoint_url(endpoint);
        let payload = self
            .send_raw(
                endpoint,
                &url,
                &[("service", Value::String(service.to_owned()))],
                false,
            )
            .await?;
        let data = codec::parse_envelope_with_codes(payload, &[-24])?;
        let object = codec::into_object(data, "service_ticket")?;
        codec::string_field(&object, "serverTicket")
            .filter(|ticket| !ticket.is_empty())
            .ok_or_else(|| {
                ProtocolError::Decode(
                    "service_ticket: response did not contain data.serverTicket".into(),
                )
            })
    }

    fn mark_expired_if_needed(&self, error: &ProtocolError) {
        let expired =
            error.is_session_expired() || matches!(error, ProtocolError::Api { code: -23, .. });
        if expired {
            self.mark_expired();
        }
    }

    fn headers(&self, anonymous: bool) -> Result<HeaderMap> {
        let tgt = if anonymous {
            String::new()
        } else {
            self.read_state().tgt
        };
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(
            HeaderName::from_static("x-requested-with"),
            HeaderValue::from_static("PC"),
        );
        headers.insert(
            HeaderName::from_static("x-app-name"),
            HeaderValue::from_static("student-pc"),
        );
        headers.insert(
            HeaderName::from_static("clientid"),
            header_value(&self.client_id)?,
        );
        headers.insert(
            HeaderName::from_static("u3r"),
            header_value(&Uuid::new_v4().simple().to_string())?,
        );
        headers.insert(HeaderName::from_static("u3t"), header_value(&tgt)?);
        headers.insert(AUTHORIZATION, HeaderValue::from_static(""));
        Ok(headers)
    }

    fn endpoint_url(&self, endpoint: &Endpoint) -> String {
        match &self.base_override {
            Some(origin) => format!("{origin}{}", endpoint.path),
            None => endpoint.url(),
        }
    }

    fn import_domain(&self) -> Option<String> {
        self.base_override
            .as_deref()
            .and_then(|origin| Url::parse(origin).ok())
            .and_then(|url| url.host_str().map(str::to_owned))
    }

    fn read_state(&self) -> ClientState {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn write_state(&self) -> std::sync::RwLockWriteGuard<'_, ClientState> {
        self.state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn system_now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0)
}

fn header_value(value: &str) -> Result<HeaderValue> {
    HeaderValue::from_str(value)
        .map_err(|error| ProtocolError::Transport(format!("invalid header value: {error}")))
}

fn truncate(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

/// Encode form fields exactly like [`crate::form::encode_form`], then percent
/// encode them as `application/x-www-form-urlencoded`.
fn encode_form_body(fields: &[(&str, Value)]) -> String {
    let mut encoder = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in fields {
        if let Some(encoded) = encode_value(value) {
            encoder.append_pair(&normalize_key(key), &encoded);
        }
    }
    encoder.finish()
}

fn append_query(url: &str, fields: &[(&str, Value)]) -> Result<String> {
    let mut parsed = Url::parse(url)
        .map_err(|error| ProtocolError::Decode(format!("invalid URL {url}: {error}")))?;
    {
        let mut pairs = parsed.query_pairs_mut();
        for (key, value) in fields {
            if let Some(encoded) = encode_value(value) {
                pairs.append_pair(&normalize_key(key), &encoded);
            }
        }
    }
    Ok(parsed.to_string())
}

#[derive(Debug, Clone)]
struct StoredCookie {
    name: String,
    value: String,
    domain: String,
    host_only: bool,
    path: String,
    secure: bool,
}

/// Minimal RFC 6265 cookie jar.
///
/// `reqwest::cookie::Jar` cannot be read back, so it cannot feed
/// [`Session::cookies`]. This implementation keeps only the fields this client
/// needs (name/value/domain/path/secure) and makes export trivial.
#[derive(Debug, Default)]
pub struct CookieJar {
    cookies: Mutex<Vec<StoredCookie>>,
}

impl CookieJar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Export cookies in insertion order, one entry per name.
    pub fn export(&self) -> Vec<(String, String)> {
        let cookies = self.lock();
        let mut exported: Vec<(String, String)> = Vec::new();
        for cookie in cookies.iter() {
            if let Some(existing) = exported.iter_mut().find(|(name, _)| name == &cookie.name) {
                existing.1.clone_from(&cookie.value);
            } else {
                exported.push((cookie.name.clone(), cookie.value.clone()));
            }
        }
        exported
    }

    /// Replace the store with imported `(name, value)` pairs.
    ///
    /// When `domain` is `None` the cookies are attached to `.up366.cn`, which
    /// covers every production host. A test/proxy override supplies its own
    /// host so imported cookies are actually sent there.
    pub fn replace(&self, pairs: &[(String, String)], domain: Option<&str>) {
        self.lock().clear();
        self.import(pairs, domain);
    }

    /// Add imported cookies without clearing existing ones.
    pub fn import(&self, pairs: &[(String, String)], domain: Option<&str>) {
        let (domain, host_only) = match domain {
            Some(host) => (host.to_ascii_lowercase(), true),
            None => ("up366.cn".to_owned(), false),
        };
        let mut cookies = self.lock();
        for (name, value) in pairs {
            Self::upsert(
                &mut cookies,
                StoredCookie {
                    name: name.clone(),
                    value: value.clone(),
                    domain: domain.clone(),
                    host_only,
                    path: "/".to_owned(),
                    secure: false,
                },
            );
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<StoredCookie>> {
        self.cookies
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn upsert(cookies: &mut Vec<StoredCookie>, cookie: StoredCookie) {
        cookies.retain(|existing| {
            !(existing.name == cookie.name
                && existing.domain == cookie.domain
                && existing.path == cookie.path)
        });
        cookies.push(cookie);
    }
}

impl CookieStore for CookieJar {
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        for header in cookie_headers {
            if let Ok(raw) = header.to_str() {
                if let Some(cookie) = parse_set_cookie(raw, url) {
                    let mut cookies = self.lock();
                    Self::upsert(&mut cookies, cookie);
                }
            }
        }
    }

    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        let host = url.host_str()?.to_ascii_lowercase();
        let path = url.path();
        let secure = url.scheme() == "https";
        let cookies = self.lock();

        let mut header = String::new();
        for cookie in cookies.iter() {
            if cookie.secure && !secure {
                continue;
            }
            let domain_ok = if cookie.host_only {
                host == cookie.domain
            } else {
                domain_matches(&host, &cookie.domain)
            };
            if !domain_ok || !path_matches(path, &cookie.path) {
                continue;
            }
            if !header.is_empty() {
                header.push_str("; ");
            }
            header.push_str(&cookie.name);
            header.push('=');
            header.push_str(&cookie.value);
        }

        if header.is_empty() {
            None
        } else {
            HeaderValue::from_str(&header).ok()
        }
    }
}

fn parse_set_cookie(raw: &str, url: &Url) -> Option<StoredCookie> {
    let mut segments = raw.split(';');
    let pair = segments.next()?;
    let (name, value) = pair.split_once('=')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let value = value.trim().trim_matches('"').to_owned();
    let host = url.host_str()?.to_ascii_lowercase();

    let mut domain = host.clone();
    let mut host_only = true;
    let mut path = default_cookie_path(url.path());
    let mut secure = false;
    let mut expired = false;

    for segment in segments {
        let segment = segment.trim();
        let (key, attribute) = segment.split_once('=').unwrap_or((segment, ""));
        match key.trim().to_ascii_lowercase().as_str() {
            "domain" => {
                let candidate = attribute
                    .trim()
                    .trim_start_matches('.')
                    .to_ascii_lowercase();
                if candidate.is_empty() || !domain_matches(&host, &candidate) {
                    return None;
                }
                domain = candidate;
                host_only = false;
            }
            "path" => {
                let candidate = attribute.trim();
                if candidate.starts_with('/') {
                    path = candidate.to_owned();
                }
            }
            "secure" => secure = true,
            "max-age" => {
                if let Ok(seconds) = attribute.trim().parse::<i64>() {
                    if seconds <= 0 {
                        expired = true;
                    }
                }
            }
            _ => {}
        }
    }

    if expired {
        return None;
    }

    Some(StoredCookie {
        name: name.to_owned(),
        value,
        domain,
        host_only,
        path,
        secure,
    })
}

fn default_cookie_path(path: &str) -> String {
    if !path.starts_with('/') {
        return "/".to_owned();
    }
    match path.rfind('/') {
        Some(0) | None => "/".to_owned(),
        Some(index) => path[..index].to_owned(),
    }
}

fn domain_matches(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

fn path_matches(request_path: &str, cookie_path: &str) -> bool {
    request_path == cookie_path
        || (request_path.starts_with(cookie_path)
            && (cookie_path.ends_with('/')
                || request_path.as_bytes().get(cookie_path.len()) == Some(&b'/')))
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use serde_json::json;

    fn session() -> Session {
        Session::new("alice", "TGT-1", "CID-TEST")
    }

    fn client(server: &MockServer) -> Client {
        Client::builder()
            .client_id("CID-TEST")
            .session(session())
            .base_url_override(server.base_url())
            .retry_base_delay(Duration::ZERO)
            .clock(|| 1_700_000_000_000)
            .build()
            .unwrap()
    }

    fn ok(data: Value) -> Value {
        json!({ "result": { "code": 0, "msg": "ok" }, "data": data })
    }

    #[test]
    fn headers_send_the_seven_fixed_values() {
        let client = Client::builder()
            .client_id("CID-TEST")
            .session(session())
            .build()
            .unwrap();
        let headers = client.headers(false).unwrap();

        assert_eq!(headers[USER_AGENT], USER_AGENT_VALUE);
        assert_eq!(headers["x-requested-with"], "PC");
        assert_eq!(headers["x-app-name"], "student-pc");
        assert_eq!(headers["clientid"], "CID-TEST");
        assert_eq!(headers["u3t"], "TGT-1");
        assert_eq!(headers[AUTHORIZATION], "");
        let u3r = headers["u3r"].to_str().unwrap();
        assert_eq!(u3r.len(), 32);
        assert!(u3r
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase()));

        let anonymous = client.headers(true).unwrap();
        assert_eq!(anonymous["u3t"], "");
    }

    #[test]
    fn form_body_and_query_encoding_match_the_client_rules() {
        let body = encode_form_body(&[
            ("status", json!(1)),
            ("pager###pageSize", json!(999)),
            ("empty", Value::Null),
            ("filter", json!({ "name": "词汇" })),
        ]);
        assert!(body.contains("status=1"));
        assert!(body.contains("pager.pageSize=999"));
        assert!(!body.contains("empty"));
        assert!(body.contains("filter=%7B%22name%22%3A%22%E8%AF%8D%E6%B1%87%22%7D"));

        assert_eq!(
            append_query("https://a.test/b", &[("x", json!("1 2"))]).unwrap(),
            "https://a.test/b?x=1+2"
        );
        assert_eq!(
            append_query("https://a.test/b?y=1", &[("ticket", json!("S&T"))]).unwrap(),
            "https://a.test/b?y=1&ticket=S%26T"
        );
    }

    #[test]
    fn cookie_jar_scopes_exported_cookies_by_domain() {
        let jar = CookieJar::new();
        let login = Url::parse("https://sso.up366.cn/v2/tickets-v3").unwrap();
        let header = HeaderValue::from_static("CASTGC=abc; Domain=.up366.cn; Path=/; HttpOnly");
        let mut headers = std::iter::once(&header);
        jar.set_cookies(&mut headers, &login);

        let business = Url::parse("https://course-api.up366.cn/x").unwrap();
        assert_eq!(
            jar.cookies(&business).unwrap().to_str().unwrap(),
            "CASTGC=abc"
        );
        assert_eq!(jar.export(), vec![("CASTGC".to_owned(), "abc".to_owned())]);

        let restored = CookieJar::new();
        restored.import(&[("UP366-C".into(), "xyz".into())], None);
        let business = Url::parse("https://book-api.up366.cn/x").unwrap();
        assert_eq!(
            restored.cookies(&business).unwrap().to_str().unwrap(),
            "UP366-C=xyz"
        );
    }

    #[tokio::test]
    async fn call_sends_form_body_and_headers() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/course/list-of-student/v2")
                .header("clientid", "CID-TEST")
                .header("u3t", "TGT-1")
                .header("user-agent", USER_AGENT_VALUE)
                .header("x-requested-with", "PC")
                .header("x-app-name", "student-pc")
                .body_contains("status=1")
                .body_contains("pager.pageSize=999");
            then.status(200).json_body(ok(json!({ "list": [] })));
        });

        let client = client(&server);
        let data = client
            .call(
                get("courses").unwrap(),
                &[("status", json!(1)), ("pager###pageSize", json!(999))],
            )
            .await
            .unwrap();

        assert_eq!(data, Some(json!({ "list": [] })));
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn read_requests_retry_twice_on_transient_status() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/client/course/list-of-student/v2");
            then.status(503).body("busy");
        });
        let client = client(&server);

        let error = client.call(get("courses").unwrap(), &[]).await.unwrap_err();

        assert!(matches!(error, ProtocolError::Http { status: 503, .. }));
        mock.assert_hits(3);
    }

    #[tokio::test]
    async fn write_requests_are_never_retried() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/client/wordsbtl/student/submit");
            then.status(500).body("boom");
        });
        let client = client(&server);

        let error = client
            .call(get("word_pk_submit").unwrap(), &[])
            .await
            .unwrap_err();

        assert!(matches!(error, ProtocolError::Http { status: 500, .. }));
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn http_404_is_reported_without_retry() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/client/course/list-of-student/v2");
            then.status(404).body("not found");
        });
        let client = client(&server);

        let error = client.call(get("courses").unwrap(), &[]).await.unwrap_err();

        match error {
            ProtocolError::Http { status, body } => {
                assert_eq!(status, 404);
                assert_eq!(body, "not found");
            }
            other => panic!("unexpected error: {other:?}"),
        }
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn malformed_json_is_a_decode_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/client/course/list-of-student/v2");
            then.status(200).body("<html>not json</html>");
        });
        let client = client(&server);

        let error = client.call(get("courses").unwrap(), &[]).await.unwrap_err();
        assert!(matches!(error, ProtocolError::Decode(_)));
    }

    #[tokio::test]
    async fn business_failure_is_not_retried() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/client/wordsbtl/student/submit");
            then.status(200)
                .json_body(json!({ "result": { "code": -13, "msg": "已提交" } }));
        });
        let client = client(&server);

        let error = client
            .call(get("word_pk_submit").unwrap(), &[])
            .await
            .unwrap_err();

        assert!(matches!(error, ProtocolError::Api { code: -13, .. }));
        assert!(matches!(
            ProtocolError::Api {
                code: -13,
                message: String::new()
            }
            .into_domain(),
            up366_core::DomainError::Duplicate
        ));
        mock.assert_hits(1);
    }

    fn without_ticket(request: &HttpMockRequest) -> bool {
        !request
            .query_params
            .as_ref()
            .is_some_and(|params| params.iter().any(|(key, _)| key == "ticket"))
    }

    #[tokio::test]
    async fn minus_21_replays_once_with_service_ticket() {
        let server = MockServer::start();
        let first = server.mock(|when, then| {
            when.method(POST)
                .path("/client/course/list-of-student/v2")
                .matches(without_ticket);
            then.status(200)
                .json_body(json!({ "result": { "code": -21, "msg": "没有登录" } }));
        });
        let replay = server.mock(|when, then| {
            when.method(POST)
                .path("/client/course/list-of-student/v2")
                .query_param("ticket", "ST-123");
            then.status(200).json_body(ok(json!({ "list": [] })));
        });
        let ticket = server.mock(|when, then| {
            when.method(POST).path("/v1/serviceTicket");
            then.status(200).json_body(json!({
                "result": { "code": -24, "msg": "ok" },
                "data": { "serverTicket": "ST-123" }
            }));
        });

        let client = client(&server);
        let data = client.call(get("courses").unwrap(), &[]).await.unwrap();

        assert_eq!(data, Some(json!({ "list": [] })));
        first.assert_hits(1);
        replay.assert_hits(1);
        ticket.assert_hits(1);
        assert!(!client.is_expired());
    }

    #[tokio::test]
    async fn service_ticket_is_requested_with_the_failed_url() {
        let server = MockServer::start();
        let first = server.mock(|when, then| {
            when.method(POST)
                .path("/client/course/list-of-student/v2")
                .matches(without_ticket);
            then.status(200)
                .json_body(json!({ "result": { "code": -22, "msg": "ticket invalid" } }));
        });
        let replay = server.mock(|when, then| {
            when.method(POST)
                .path("/client/course/list-of-student/v2")
                .query_param("ticket", "ST-9");
            then.status(200).json_body(ok(json!({ "list": [] })));
        });
        let ticket = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/serviceTicket")
                .body_contains("service=");
            then.status(200).json_body(json!({
                "result": { "code": -24, "msg": "ok" },
                "data": { "serverTicket": "ST-9" }
            }));
        });

        let client = client(&server);
        client.call(get("courses").unwrap(), &[]).await.unwrap();

        first.assert_hits(1);
        replay.assert_hits(1);
        ticket.assert_hits(1);
    }

    #[tokio::test]
    async fn a_failed_replay_is_not_retried_again() {
        let server = MockServer::start();
        let first = server.mock(|when, then| {
            when.method(POST)
                .path("/client/course/list-of-student/v2")
                .matches(without_ticket);
            then.status(200)
                .json_body(json!({ "result": { "code": -21, "msg": "no login" } }));
        });
        let replay = server.mock(|when, then| {
            when.method(POST)
                .path("/client/course/list-of-student/v2")
                .query_param("ticket", "ST-1");
            then.status(200)
                .json_body(json!({ "result": { "code": -21, "msg": "still no login" } }));
        });
        let ticket = server.mock(|when, then| {
            when.method(POST).path("/v1/serviceTicket");
            then.status(200).json_body(json!({
                "result": { "code": -24, "msg": "ok" },
                "data": { "serverTicket": "ST-1" }
            }));
        });

        let client = client(&server);
        let error = client.call(get("courses").unwrap(), &[]).await.unwrap_err();

        assert!(matches!(error, ProtocolError::Api { code: -21, .. }));
        first.assert_hits(1);
        replay.assert_hits(1);
        ticket.assert_hits(1);
        assert!(client.is_expired());
    }

    #[tokio::test]
    async fn a_business_host_reporting_no_tgt_marks_the_session_expired() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST)
                .path("/client/course/list-of-student/v2")
                .matches(without_ticket);
            then.status(200)
                .json_body(json!({ "result": { "code": -23, "msg": "找不到tgt" } }));
        });

        let client = client(&server);
        let error = client.call(get("courses").unwrap(), &[]).await.unwrap_err();

        assert!(matches!(error, ProtocolError::Api { code: -23, .. }));
        assert!(client.is_expired());
    }

    #[tokio::test]
    async fn a_rejected_ticket_exchange_marks_the_session_expired() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST)
                .path("/client/course/list-of-student/v2")
                .matches(without_ticket);
            then.status(200)
                .json_body(json!({ "result": { "code": -21, "msg": "没有登录" } }));
        });
        server.mock(|when, then| {
            when.method(POST).path("/v1/serviceTicket");
            then.status(200)
                .json_body(json!({ "result": { "code": -4, "msg": "找不到tgt" } }));
        });

        let client = client(&server);
        let error = client.call(get("courses").unwrap(), &[]).await.unwrap_err();

        assert!(error.to_string().contains("找不到tgt"));
        assert!(client.is_expired());
    }
}
