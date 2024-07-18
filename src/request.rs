use std::cell::OnceCell;
use std::collections::HashMap;
use std::convert::TryInto;
use std::fmt::{Display, Formatter};
use std::io;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::num::ParseIntError;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use flate2::read::GzDecoder;
use lazy_static::lazy_static;
use thiserror::Error;

use crate::url::{Scheme, UrlError, WebUrl};

const USER_AGENT: &str = "Octo";

lazy_static! {
    static ref ROOT_STORE: Arc<rustls::RootCertStore> = Arc::new(rustls::RootCertStore::from_iter(
        webpki_roots::TLS_SERVER_ROOTS.iter().cloned()
    ));
    static ref CONFIG: Arc<rustls::ClientConfig> = Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(ROOT_STORE.clone())
            .with_no_client_auth()
    );
}

#[derive(Error, Debug)]
pub(crate) enum RequestError {
    #[error("invalid scheme for a web URL: {0}")]
    InvalidScheme(Scheme),

    #[error("can't connect via TCP")]
    ConnectionFailed(#[from] io::Error),
}

#[derive(Error, Debug)]
pub enum ResponseError {
    #[error("missing status line: {0}")]
    MissingStatusLine(String),

    #[error("invalid status line: {0}")]
    InvalidStatusLine(String),

    #[error("failed to parse the status code: {0}")]
    InvalidStatusCode(#[from] ParseIntError),

    #[error("failed to parse the headers: {0}")]
    Headers(String),

    #[error("error reading the response stream: {0}")]
    Stream(#[from] io::Error),
}

#[derive(Error, Debug)]
#[error(transparent)]
pub struct BrowserError(#[from] NetworkError);

#[derive(Error, Debug)]
pub(crate) enum NetworkError {
    #[error(transparent)]
    Url(#[from] UrlError),

    #[error(transparent)]
    Request(#[from] RequestError),

    #[error(transparent)]
    Response(#[from] ResponseError),
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum RequestMethod {
    Get,
}

impl Display for RequestMethod {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Get => write!(f, "GET"),
        }
    }
}

/// Abstraction over both `std::net::TcpStream` and `rustls::StreamOwned`
#[derive(Debug)]
enum GenericTcpStream {
    Insecure(TcpStream),
    Secure(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl GenericTcpStream {
    fn connect_insecure(url: &WebUrl) -> Result<Self> {
        let stream = TcpStream::connect(format!("{}:{}", url.host, url.port))?;
        Ok(Self::Insecure(stream))
    }

    fn connect_secure(url: &WebUrl) -> Result<Self> {
        let stream = TcpStream::connect(format!("{}:{}", url.host, url.port))?;
        let client = rustls::ClientConnection::new(CONFIG.clone(), url.host.clone().try_into()?)?;
        let tls = rustls::StreamOwned::new(client, stream);
        Ok(Self::Secure(Box::new(tls)))
    }
}

impl Read for GenericTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Insecure(stream) => stream.read(buf),
            Self::Secure(stream) => stream.read(buf),
        }
    }
}

impl Write for GenericTcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Insecure(stream) => stream.write(buf),
            Self::Secure(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Insecure(stream) => stream.flush(),
            Self::Secure(stream) => stream.flush(),
        }
    }
}

// Have to make a newtype because OnceCell::get_mut_or_init
// isn't available on stable, and we need to put the TcpStream in a OnceCell
// so that it's not dropped (and therefore closed) after every call to Request.make
#[derive(Debug)]
struct ReusableTcpStream(OnceCell<GenericTcpStream>);

impl ReusableTcpStream {
    fn new() -> Self {
        Self(OnceCell::new())
    }

    fn get_mut_or_try_init<F>(&mut self, f: F) -> Result<&mut GenericTcpStream>
    where
        F: FnOnce() -> Result<GenericTcpStream>,
    {
        // There might be a more elegant way of doing this,
        // but this satisfies the borrow checker, and is good enough for now.
        if self.0.get().is_none() {
            let stream = f()?;
            self.0.set(stream).unwrap();
        }
        Ok(self.0.get_mut().unwrap())
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Headers {
    headers: HashMap<String, Vec<String>>,
}

impl Headers {
    fn add_header_values(&mut self, key: &str, values: &[&str]) {
        let key = key.to_lowercase();

        // TODO: Check the spec to make sure we actually want to filter out empty strings
        // from values here.
        let values = values
            .iter()
            .filter_map(|s| {
                if !s.is_empty() {
                    Some(s.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if let Some(existing_values) = self.headers.get_mut(&key) {
            for value in values {
                existing_values.push(value.to_string());
            }
        } else {
            self.headers.insert(key, values);
        }
    }

    fn add_many(&mut self, kv_pairs: &[(&str, &[&str])]) {
        for (key, values) in kv_pairs {
            self.add_header_values(*key, *values);
        }
    }

    /// Adds one header key/value pair, where the value is a single header value.
    /// Example: `add_one_pair("content-encoding", "gzip")`
    /// **Note:** The header **key** will be converted to lowercase, but the **value** will not.
    /// Does not perform any deduplication.
    fn add(&mut self, key: &str, value: &str) {
        self.add_header_values(key, &[value]);
    }

    pub fn get(&self, key: &str) -> Option<&Vec<String>> {
        self.headers.get(key)
    }

    /// Returns true if `Headers` contains the given header with non-empty values.
    fn contains(&self, key: &str) -> bool {
        if let Some(values) = self.headers.get(key) {
            !values.is_empty()
        } else {
            false
        }
    }

    /// Returns `Some(true)` if the given header key is associated with the given value,
    /// `Some(false)` if the given header is not associated with the given value,
    /// and `None` if the given header is not in `Headers` at all.
    fn has_value(&self, key: &str, value: &str) -> Option<bool> {
        self.headers
            .get(key)
            .map(|values| values.iter().any(|s| s.as_str() == value))
    }

    fn from(kv_pairs: &[(&str, &[&str])]) -> Self {
        let mut headers = Headers::default();
        headers.add_many(kv_pairs);
        headers
    }
}

impl Display for Headers {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        for (key, values) in &self.headers {
            let values = values.join(", ");
            s.push_str(format!("{key}: {values}\r\n").as_str());
        }
        write!(f, "{s}")
    }
}

#[derive(Debug)]
pub(crate) struct Request {
    method: RequestMethod,
    headers: Headers,
    // TODO: Switch to &WebUrl to avoid taking ownership/cloning?
    stream: ReusableTcpStream,
    keep_alive: bool,
}

impl Request {
    pub(crate) fn new(method: RequestMethod, host: &str, keep_alive: bool) -> Self {
        let connection_value = if keep_alive { "keep-alive" } else { "close" };
        Self {
            method,
            headers: Headers::from(&[
                ("Host", &[host]),
                ("Connection", &[connection_value]),
                ("Compression", &["gzip"]),
            ]),
            stream: ReusableTcpStream::new(),
            keep_alive,
        }
    }

    /// Adds given Header key/values to the Request.
    /// The same Header key can be specified multiple times.
    /// Note that this does not overwrite any existing headers!
    /// If a given Header already exists in this Request,
    /// the new value(s) will simply be appended to that Header.
    pub(crate) fn with_extra_headers(mut self, headers: &[(&str, &[&str])]) -> Self {
        self.headers.add_many(headers);
        self
    }

    fn make_string(&self, url: &WebUrl, _body: Option<&str>) -> String {
        let mut string = format!("{} {} HTTP/1.1\r\n", self.method, url.path);
        string.push_str(self.headers.to_string().as_str());
        // TODO add body
        string.push_str("\r\n");
        string
    }

    pub(crate) fn make(&mut self, url: &WebUrl, body: Option<&str>) -> Result<Response> {
        if !matches!(url.scheme, Scheme::Http) && !matches!(url.scheme, Scheme::Https) {
            return Err(RequestError::InvalidScheme(url.scheme).into());
        }
        let self_string = self.make_string(url, body);

        let stream = self.stream.get_mut_or_try_init(|| {
            if matches!(url.scheme, Scheme::Http) {
                GenericTcpStream::connect_insecure(url)
            } else {
                // HTTPS
                GenericTcpStream::connect_secure(url)
            }
        })?;
        stream.write_all(self_string.as_bytes())?;

        if self.keep_alive {
            Ok(Response::from_stream(stream)?)
        } else {
            let mut response_data = String::new();
            stream.read_to_string(&mut response_data)?;
            Ok(response_data.parse::<Response>()?)
        }
    }
}

impl Request {
    /// Convenience method to make a GET request
    /// to the given URL with the defaylt `User-Agent`
    /// and return the resulting `Response` or error.
    pub(crate) fn get(url: &WebUrl) -> Result<Response> {
        let mut request = Self::new(RequestMethod::Get, &url.host, false)
            .with_extra_headers(&[("User-Agent", &[USER_AGENT])]);
        request.make(url, None)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StatusLine {
    pub version: String,
    pub status_code: u16,
    pub explanation: String,
}

impl FromStr for StatusLine {
    type Err = ResponseError;

    fn from_str(s: &str) -> Result<Self, ResponseError> {
        let (version, status, explanation) = {
            let parts = s.splitn(3, ' ').collect::<Vec<_>>();
            if parts.len() < 3 {
                return Err(ResponseError::InvalidStatusLine(s.to_string()));
            }
            (parts[0], parts[1], parts[2].trim())
        };
        let status_code = status.parse::<u16>()?;
        Ok(Self {
            version: version.to_string(),
            status_code,
            explanation: explanation.to_string(),
        })
    }
}

#[inline]
fn decompress_gzip(bytes: &[u8]) -> Result<String, ResponseError> {
    let mut gz = GzDecoder::new(bytes);
    let mut string = String::new();
    gz.read_to_string(&mut string)?;
    Ok(string)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    status_line: StatusLine,
    pub headers: Headers,
    pub body: Option<String>,
}

impl Response {
    fn from_stream(stream: &mut impl Read) -> Result<Self, ResponseError> {
        let mut reader = BufReader::new(stream);
        let mut status_line = String::new();
        reader.read_line(&mut status_line)?;
        let status_line = status_line.parse::<StatusLine>()?;

        // TODO: Support multiple header values for the same header key
        let mut headers = Headers::default();
        let mut current_line = String::new();

        loop {
            reader.read_line(&mut current_line)?;
            if current_line.as_str() == "\r\n" {
                break;
            }

            let (header, values) = current_line
                .split_once(':')
                .ok_or_else(|| ResponseError::Headers(current_line.clone()))?;
            let values = values.split(',').map(|s| s.trim()).collect::<Vec<_>>();
            headers.add_header_values(header, &values);
            current_line.clear();
        }

        let content_length = headers
            .get("content-length")
            .and_then(|values| values.get(0))
            .map(|s| s.parse::<usize>())
            .transpose()?
            .unwrap_or(0);

        let mut buf = Vec::with_capacity(content_length);

        // let mut body_buf = String::with_capacity(content_length);
        let mut bytes_read = 0;
        while bytes_read < content_length {
            let new_bytes_read = reader.read(&mut buf)?;
            if new_bytes_read == 0 {
                break;
            }
            bytes_read += new_bytes_read;
        }
        let body = if !buf.is_empty() {
            let body = if headers.has_value("content-encoding", "gzip") == Some(true) {
                decompress_gzip(&buf)?
            } else {
                String::from_utf8_lossy(&buf).to_string()
            };
            Some(body)
        } else {
            None
        };

        Ok(Self {
            status_line,
            headers,
            body,
        })
    }

    pub(crate) fn status_code(&self) -> u16 {
        self.status_line.status_code
    }
}

impl FromStr for Response {
    type Err = ResponseError;

    fn from_str(s: &str) -> Result<Self, ResponseError> {
        Self::from_stream(&mut s.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::url::Url;

    #[test]
    fn close() {
        let url = "http://example.com".parse::<Url>().unwrap();
        let response = Request::get(url.as_web_url().unwrap()).unwrap();
        assert!(response.body.is_some());
    }

    fn test_url_keepalive(url: &str) {
        let url = url.parse::<Url>().unwrap();
        let url = url.as_web_url().unwrap();

        let mut request = Request::new(RequestMethod::Get, &url.host, true);
        let first_response = request.make(url, None).unwrap();
        assert!(first_response.body.is_some());
        let second_response = request.make(url, None).unwrap();
        assert_eq!(first_response, second_response);

        let one_off_response = Request::get(url).unwrap();
        assert_eq!(first_response.body, one_off_response.body);
        assert_eq!(second_response.body, one_off_response.body);
    }

    #[test]
    fn keep_alive() {
        test_url_keepalive("http://example.com");
    }

    #[test]
    fn keep_alive_http() {
        test_url_keepalive("https://example.com");
    }
}
