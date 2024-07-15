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
pub(crate) enum ResponseError {
    #[error("missing status line: {0}")]
    MissingStatusLine(String),

    #[error("invalid status line: {0}")]
    InvalidStatusLine(String),

    #[error("failed to parse the status code: {0}")]
    InvalidStatusCode(#[from] ParseIntError),

    #[error("failed to parse the headers: {0}")]
    Headers(String),
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
    Secure(rustls::StreamOwned<rustls::ClientConnection, TcpStream>),
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
        Ok(Self::Secure(tls))
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

#[derive(Debug)]
pub(crate) struct Request {
    method: RequestMethod,
    headers: HashMap<String, Vec<String>>,
    body: Option<String>,
    // TODO: Switch to &WebUrl to avoid taking ownership/cloning?
    url: WebUrl,
    stream: ReusableTcpStream,
    keep_alive: bool,
}

impl Request {
    pub(crate) fn init(method: RequestMethod, url: WebUrl, keep_alive: bool) -> Self {
        let connection_value = if keep_alive { "keep-alive" } else { "close" };
        Self {
            method,
            headers: HashMap::from([
                ("Host".to_string(), vec![url.host.clone()]),
                ("Connection".to_string(), vec![connection_value.to_string()]),
            ]),
            body: None,
            url,
            stream: ReusableTcpStream::new(),
            keep_alive,
        }
    }

    /// Adds given Header key/values to the Request.
    /// The same Header key can be specified multiple times.
    /// Note that this does not overwrite any existing headers!
    /// If a given Header already exists in this Request,
    /// the new value(s) will simply be appended to that Header.
    pub(crate) fn with_extra_headers(mut self, headers: &[(&str, &str)]) -> Self {
        for (header, value) in headers {
            if let Some(existing_values) = self.headers.get_mut(*header) {
                existing_values.push(value.to_string());
            } else {
                self.headers
                    .insert(header.to_string(), vec![value.to_string()]);
            }
        }
        self
    }

    pub(crate) fn with_body(mut self, body: &str) -> Self {
        self.body = Some(body.to_string());
        self
    }

    pub(crate) fn make(&mut self) -> Result<Response> {
        if !matches!(self.url.scheme, Scheme::Http) && !matches!(self.url.scheme, Scheme::Https) {
            return Err(RequestError::InvalidScheme(self.url.scheme).into());
        }
        let self_string = self.to_string();

        let stream = self.stream.get_mut_or_try_init(|| {
            if matches!(self.url.scheme, Scheme::Http) {
                GenericTcpStream::connect_insecure(&self.url)
            } else {
                // HTTPS
                GenericTcpStream::connect_secure(&self.url)
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
        let mut request = Self::init(RequestMethod::Get, url.clone(), false)
            .with_extra_headers(&[("User-Agent", USER_AGENT)]);
        request.make()
    }
}

impl Display for Request {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut string = format!("{} {} HTTP/1.1\r\n", self.method, self.url.path);
        for (header, values) in self.headers.iter() {
            for value in values {
                string.push_str(format!("{header}: {value}\r\n").as_str());
            }
        }
        // TODO add body
        write!(f, "{string}\r\n")
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
            (parts[0], parts[1], parts[2])
        };
        let status_code = status.parse::<u16>()?;
        Ok(Self {
            version: version.to_string(),
            status_code,
            explanation: explanation.to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Response {
    pub status_line: StatusLine,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl Response {
    fn from_stream(stream: &mut GenericTcpStream) -> Result<Self> {
        // TODO: Use io::BufReader to increase efficiency?
        let mut reader = BufReader::new(stream);
        let mut status_line = String::new();
        reader.read_line(&mut status_line)?;
        let status_line = status_line.parse::<StatusLine>()?;

        // TODO: Support multiple header values for the same header key
        let mut headers = HashMap::new();
        let mut current_line = String::new();

        loop {
            reader.read_line(&mut current_line)?;
            if current_line.as_str() == "\r\n" {
                break;
            }

            let (header, value) = current_line
                .split_once(':')
                .ok_or_else(|| ResponseError::Headers(current_line.clone()))?;
            headers.insert(header.to_lowercase(), value.trim().to_string());
            current_line.clear();
        }

        let content_length = headers
            .get("content-length")
            .map(|s| s.parse::<usize>())
            .transpose()?
            .unwrap_or(0);

        let mut body_buf = String::with_capacity(content_length);
        let mut bytes_read = 0;
        while bytes_read < content_length {
            let new_bytes_read = reader.read_line(&mut body_buf)?;
            if new_bytes_read == 0 {
                break;
            }
            bytes_read += new_bytes_read;
        }
        let body = if body_buf.is_empty() {
            None
        } else {
            Some(body_buf)
        };

        Ok(Self {
            status_line,
            headers,
            body,
        })
    }
}

impl FromStr for Response {
    type Err = ResponseError;

    fn from_str(s: &str) -> Result<Self, ResponseError> {
        let mut lines = s.lines();
        let status_line = lines
            .next()
            .ok_or_else(|| ResponseError::MissingStatusLine(s.to_string()))?;
        let status_line = StatusLine::from_str(status_line)?;

        let headers: HashMap<_, _> = HashMap::from_iter(lines.by_ref().map_while(|line| {
            let (header, value) = line.split_once(':')?;
            Some((header.to_lowercase(), value.to_string()))
        }));

        assert!(!headers.contains_key("transfer-encoding"));
        assert!(!headers.contains_key("content-encoding"));

        let mut body = String::with_capacity(s.len());

        for line in lines {
            body.push_str(line);
            body.push('\n');
        }

        Ok(Self {
            status_line,
            headers,
            body: Some(body),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::url::Url;

    impl Url {
        fn as_web_url(&self) -> &WebUrl {
            match self {
                Self::Web(url) => url,
                _ => panic!("Not a WebUrl: {self:?}"),
            }
        }
    }

    #[test]
    fn close() {
        let url = "http://example.com".parse::<Url>().unwrap();
        let response = Request::get(url.as_web_url()).unwrap();
        assert!(response.body.is_some());
    }

    #[test]
    fn keep_alive() {
        let url = "http://example.com".parse::<Url>().unwrap();

        let mut request = Request::init(RequestMethod::Get, url.as_web_url().to_owned(), true);
        let first_response = request.make().unwrap();
        assert!(first_response.body.is_some());
        let second_response = request.make().unwrap();
        assert_eq!(first_response, second_response);

        let one_off_response = Request::get(url.as_web_url()).unwrap();
        assert_eq!(first_response.body, one_off_response.body);
        assert_eq!(second_response.body, one_off_response.body);
    }

    #[test]
    fn keep_alive_https() {
        let url = "https://example.com".parse::<Url>().unwrap();

        let mut request = Request::init(RequestMethod::Get, url.as_web_url().to_owned(), true);
        let first_response = request.make().unwrap();
        assert!(first_response.body.is_some());
        let second_response = request.make().unwrap();
        assert_eq!(first_response, second_response);

        let one_off_response = Request::get(url.as_web_url()).unwrap();
        assert_eq!(first_response.body, one_off_response.body);
        assert_eq!(second_response.body, one_off_response.body);
    }

    #[test]
    fn redirect() {
        let url = "https://browser.engineering/redirect"
            .parse::<Url>()
            .unwrap();
        let mut request = Request::init(RequestMethod::Get, url.as_web_url().to_owned(), true);
        let response_redirect = request.make().unwrap();

        let url_no_redirect = "https://browser.engineering/http.html"
            .parse::<Url>()
            .unwrap();
        let response_no_redirect = Request::get(url_no_redirect.as_web_url()).unwrap();
        assert_eq!(response_redirect.body, response_no_redirect.body);
    }

    #[test]
    fn redirect_2() {
        let url = "https://browser.engineering/redirect2"
            .parse::<Url>()
            .unwrap();
        let mut request = Request::init(RequestMethod::Get, url.as_web_url().to_owned(), true);
        let response_redirect = request.make().unwrap();

        let url_no_redirect = "https://browser.engineering/http.html"
            .parse::<Url>()
            .unwrap();
        let response_no_redirect = Request::get(url_no_redirect.as_web_url()).unwrap();
        assert_eq!(response_redirect.body, response_no_redirect.body);
    }

    #[test]
    fn redirect_3() {
        let url = "https://browser.engineering/redirect3"
            .parse::<Url>()
            .unwrap();
        let mut request = Request::init(RequestMethod::Get, url.as_web_url().to_owned(), true);
        let response_redirect = request.make().unwrap();

        let url_no_redirect = "https://browser.engineering/http.html"
            .parse::<Url>()
            .unwrap();
        let response_no_redirect = Request::get(url_no_redirect.as_web_url()).unwrap();
        assert_eq!(response_redirect.body, response_no_redirect.body);
    }
}
