use std::collections::HashMap;
use std::convert::TryInto;
use std::fmt::{Display, Formatter};
use std::io;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::num::ParseIntError;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use lazy_static::lazy_static;
use thiserror::Error;

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
pub(crate) enum UrlError {
    #[error("error splitting the URL: `{0}`")]
    Split(String),

    #[error("unknown URL scheme: {0}")]
    UnknownScheme(String),

    #[error("invalid scheme for an HTTP request: {0}")]
    InvalidScheme(Scheme),

    #[error("failed to parse the port: {0}")]
    InvalidPort(#[from] ParseIntError),
}

#[derive(Error, Debug)]
pub(crate) enum RequestError {
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
pub(crate) enum Scheme {
    Http,
    Https,
    File,
}

impl Scheme {
    fn default_port(&self) -> Option<u16> {
        match self {
            Self::Http => Some(80),
            Self::Https => Some(443),
            _ => None,
        }
    }
}

impl FromStr for Scheme {
    type Err = UrlError;

    fn from_str(s: &str) -> Result<Self, UrlError> {
        match s {
            "http" => Ok(Self::Http),
            "https" => Ok(Self::Https),
            "file" => Ok(Self::File),
            _ => Err(UrlError::UnknownScheme(s.to_string())),
        }
    }
}

impl Display for Scheme {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // This feels like I am abusing the Debug representation, but it works.
        let variant_name = format!("{self:?}")
            .strip_prefix("Scheme::")
            .unwrap()
            .to_lowercase();
        write!(f, "{variant_name}")
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Url {
    pub scheme: Scheme,
    pub path: String,
    pub host: String,
    pub port: Option<u16>,
}

impl FromStr for Url {
    type Err = UrlError;

    fn from_str(url: &str) -> Result<Self, UrlError> {
        let (scheme, url) = url
            .split_once("://")
            .ok_or_else(|| UrlError::Split(url.to_string()))?;
        let scheme = Scheme::from_str(scheme)?;

        let url = if url.contains('/') {
            url.to_string()
        } else {
            format!("{url}/")
        };

        let (host, url) = url
            .split_once('/')
            .ok_or_else(|| UrlError::Split(url.to_string()))?;
        let path = format!("/{url}");

        let (host, port) = if let Some((new_host, port_str)) = host.split_once(':') {
            (new_host, Some(port_str.parse::<u16>()?))
        } else {
            (host, scheme.default_port())
        };

        Ok(Self {
            scheme,
            host: host.to_string(),
            path,
            port,
        })
    }
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

#[derive(Debug, Clone)]
pub(crate) struct Request {
    method: RequestMethod,
    headers: HashMap<String, Vec<String>>,
    body: Option<String>,
    url: Url,
}

impl Request {
    pub(crate) fn init(method: RequestMethod, url: Url) -> Self {
        Self {
            method,
            headers: HashMap::from([
                ("Host".to_string(), vec![url.host.clone()]),
                ("Connection".to_string(), vec!["close".to_string()]),
            ]),
            body: None,
            url,
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

    pub(crate) fn make(&self) -> Result<Response> {
        let read_buf = {
            let mut read_buf = String::new();
            match self.url.scheme {
                Scheme::Http => {
                    let mut stream = TcpStream::connect(format!(
                        "{}:{}",
                        self.url.host,
                        self.url.port.unwrap()
                    ))?;
                    stream.write_all(self.to_string().as_bytes())?;
                    stream.read_to_string(&mut read_buf)?;
                }
                Scheme::Https => {
                    let mut client = rustls::ClientConnection::new(
                        CONFIG.clone(),
                        self.url.host.clone().try_into()?,
                    )?;

                    let mut stream = TcpStream::connect(format!(
                        "{}:{}",
                        self.url.host,
                        self.url.port.unwrap()
                    ))?;
                    let mut tls = rustls::Stream::new(&mut client, &mut stream);
                    tls.write_all(self.to_string().as_bytes())?;
                    tls.read_to_string(&mut read_buf)?;
                }
                _ => return Err(UrlError::InvalidScheme(self.url.scheme).into()),
            }
            read_buf
        };
        Ok(Response::from_str(&read_buf)?)
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub(crate) struct Response {
    pub status_line: StatusLine,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl FromStr for Response {
    type Err = ResponseError;

    fn from_str(string: &str) -> Result<Self, ResponseError> {
        let mut lines = string.lines();
        let status_line = lines
            .next()
            .ok_or_else(|| ResponseError::MissingStatusLine(string.to_string()))?;
        let status_line = StatusLine::from_str(status_line)?;

        let headers: HashMap<_, _> = HashMap::from_iter(lines.by_ref().map_while(|line| {
            if line == r"\r\n" {
                None
            } else {
                let (header, value) = line.split_once(':')?;
                Some((header.to_lowercase(), value.trim().to_string()))
            }
        }));

        assert!(!headers.contains_key("transfer-encoding"));
        assert!(!headers.contains_key("content-encoding"));

        let content = String::from_iter(lines);
        Ok(Self {
            status_line,
            headers,
            body: Some(content),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url() {
        let url = Url::from_str("http://example.org").unwrap();
        assert!(matches!(url.scheme, Scheme::Http));
        assert_eq!(url.host, "example.org");
        assert_eq!(url.path, "/");
        assert_eq!(url.port, Some(80));
    }

    #[test]
    fn parse_url_https() {
        let url = Url::from_str("https://example.org").unwrap();
        assert!(matches!(url.scheme, Scheme::Https));
        assert_eq!(url.host, "example.org");
        assert_eq!(url.path, "/");
        assert_eq!(url.port, Some(443));
    }

    #[test]
    fn parse_url_custom_port() {
        let url = Url::from_str("https://example.org:8000").unwrap();
        assert!(matches!(url.scheme, Scheme::Https));
        assert_eq!(url.host, "example.org");
        assert_eq!(url.path, "/");
        assert_eq!(url.port, Some(8000));
    }
}
