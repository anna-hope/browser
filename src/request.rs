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
    Data,
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

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http" => Ok(Self::Http),
            "https" => Ok(Self::Https),
            "file" => Ok(Self::File),
            "data" => Ok(Self::Data),
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
pub(crate) enum Url {
    Web(WebUrl),
    File(FileUrl),
    Data(DataUrl),
}

impl Url {
    pub(crate) fn scheme(&self) -> Scheme {
        match self {
            Self::Web(url) => url.scheme,
            Self::File(url) => url.scheme,
            Self::Data(url) => url.scheme,
        }
    }

    pub(crate) fn path(&self) -> Option<&str> {
        match self {
            Self::Web(url) => Some(url.path.as_str()),
            Self::File(url) => Some(url.path.as_str()),
            _ => None,
        }
    }

    pub(crate) fn host(&self) -> Option<&str> {
        match self {
            Self::Web(url) => Some(url.host.as_str()),
            Self::File(url) => Some(url.host.as_str()),
            _ => None,
        }
    }

    pub(crate) fn port(&self) -> Option<u16> {
        match self {
            Self::Web(url) => Some(url.port),
            _ => None,
        }
    }
}

impl FromStr for Url {
    type Err = UrlError;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        let (scheme, url) = url
            .split_once(':')
            .ok_or_else(|| UrlError::Split(url.to_string()))?;
        let scheme = Scheme::from_str(scheme)?;
        if matches!(scheme, Scheme::Data) {
            return Ok(Self::Data(url.parse::<DataUrl>()?));
        }
        let url = url
            .strip_prefix("//")
            .ok_or_else(|| UrlError::Split(url.to_string()))?;

        let url = if url.contains('/') {
            url.to_string()
        } else {
            format!("{url}/")
        };

        let (host, url) = url
            .split_once('/')
            .ok_or_else(|| UrlError::Split(url.to_string()))?;
        let path = format!("/{url}");

        match scheme {
            Scheme::Http | Scheme::Https => {
                let (host, port) = if let Some((new_host, port_str)) = host.split_once(':') {
                    (new_host, port_str.parse::<u16>()?)
                } else {
                    // Http and Https are guaranteed to have a default port, so safe to unwrap.
                    (host, scheme.default_port().unwrap())
                };
                Ok(Self::Web(WebUrl {
                    scheme,
                    host: host.to_string(),
                    path,
                    port,
                }))
            }
            Scheme::File => Ok(Self::File(FileUrl {
                scheme,
                host: host.to_string(),
                path,
            })),
            // We handled this above, so this will never happen.
            Scheme::Data => unreachable!(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WebUrl {
    pub scheme: Scheme,
    pub path: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub(crate) struct FileUrl {
    pub scheme: Scheme,
    pub path: String,
    pub host: String,
}

#[derive(Debug, Clone)]
pub(crate) struct DataUrl {
    pub scheme: Scheme,
    // TODO: Use enumerated mimetypes instead of String
    pub mimetype: String,
    // TODO: Add base64 bool field
    pub data: String,
}

impl FromStr for DataUrl {
    type Err = UrlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // TODO: Currently doesn't handle parsing the optional base64 token.
        let (mimetype, data) = s
            .split_once(',')
            .ok_or_else(|| UrlError::Split(s.to_string()))?;
        Ok(Self {
            scheme: Scheme::Data,
            mimetype: mimetype.to_string(),
            data: data.to_string(),
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
    url: WebUrl,
}

impl Request {
    pub(crate) fn init(method: RequestMethod, url: WebUrl) -> Self {
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
                    let mut stream =
                        TcpStream::connect(format!("{}:{}", self.url.host, self.url.port,))?;
                    stream.write_all(self.to_string().as_bytes())?;
                    stream.read_to_string(&mut read_buf)?;
                }
                Scheme::Https => {
                    let mut client = rustls::ClientConnection::new(
                        CONFIG.clone(),
                        self.url.host.clone().try_into()?,
                    )?;

                    let mut stream =
                        TcpStream::connect(format!("{}:{}", self.url.host, self.url.port,))?;
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
        let url = "http://example.org".parse::<Url>().unwrap();
        assert!(matches!(url.scheme(), Scheme::Http));
        assert_eq!(url.host().unwrap(), "example.org");
        assert_eq!(url.path().unwrap(), "/");
        assert_eq!(url.port().unwrap(), 80);
    }

    #[test]
    fn parse_url_https() {
        let url = "https://example.org".parse::<Url>().unwrap();
        assert!(matches!(url.scheme(), Scheme::Https));
        assert_eq!(url.host().unwrap(), "example.org");
        assert_eq!(url.path().unwrap(), "/");
        assert_eq!(url.port().unwrap(), 443);
    }

    #[test]
    fn parse_url_custom_port() {
        let url = "https://example.org:8000".parse::<Url>().unwrap();
        assert!(matches!(url.scheme(), Scheme::Https));
        assert_eq!(url.host().unwrap(), "example.org");
        assert_eq!(url.path().unwrap(), "/");
        assert_eq!(url.port().unwrap(), 8000);
    }

    #[test]
    fn parse_data_url() {
        let url = "data:text/html,Hello world!".parse::<Url>().unwrap();
        match url {
            Url::Data(url) => {
                assert!(matches!(url.scheme, Scheme::Data));
                assert_eq!(url.mimetype, "text/html");
                assert_eq!(url.data, "Hello world!");
            }
            _ => panic!("Expected a DataUrl, got {url:?}"),
        }
    }
}
