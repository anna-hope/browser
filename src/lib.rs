use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

use anyhow::{Context, Result};
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
pub enum URLError {
    #[error("error splitting the URL: `{0}`")]
    Split(String),

    #[error("can't connect via TCP")]
    ConnectionFailed(#[from] io::Error),

    #[error("invalid TCP response: {0}")]
    InvalidResponse(String),

    #[error("unknown URL scheme: {0}")]
    UnknownScheme(String),
}

#[derive(Debug, Copy, Clone)]
pub enum Scheme {
    HTTP,
    HTTPS,
}

impl Scheme {
    fn from_str(scheme: &str) -> Result<Self, URLError> {
        match scheme {
            "http" => Ok(Self::HTTP),
            "https" => Ok(Self::HTTPS),
            _ => Err(URLError::UnknownScheme(scheme.to_string())),
        }
    }

    fn default_port(&self) -> u16 {
        match self {
            Self::HTTP => 80,
            Self::HTTPS => 443,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum RequestMethod {
    GET,
}

impl Display for RequestMethod {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GET => write!(f, "GET"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: RequestMethod,
    headers: HashMap<String, Vec<String>>,
    pub body: Option<String>,
    pub url: URL,
}

impl Request {
    fn init(method: RequestMethod, url: URL) -> Self {
        Self {
            method,
            headers: HashMap::from([("Host".to_string(), vec![url.host.clone()])]),
            body: None,
            url,
        }
    }

    /// Adds given Header key/values to the Request.
    /// The same Header key can be specified multiple times.
    /// Note that this does not overwrite any existing headers!
    /// If a given Header already exists in this Request,
    /// the new value(s) will simply be appended to that Header.
    fn add_headers(mut self, headers: &[(&str, &str)]) -> Self {
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

    fn with_body(mut self, body: &str) -> Self {
        self.body = Some(body.to_string());
        self
    }

    fn make(&self) -> Result<Response> {
        let read_buf = {
            let mut stream = TcpStream::connect(format!("{}:{}", self.url.host, self.url.port))?;
            let mut read_buf = String::new();
            match self.url.scheme {
                Scheme::HTTP => {
                    stream.write_all(self.to_string().as_bytes())?;
                    stream.read_to_string(&mut read_buf)?;
                }
                Scheme::HTTPS => {
                    let mut client = rustls::ClientConnection::new(
                        CONFIG.clone(),
                        self.url.host.clone().try_into()?,
                    )?;

                    let mut tls = rustls::Stream::new(&mut client, &mut stream);
                    tls.write_all(self.to_string().as_bytes())?;
                    tls.read_to_string(&mut read_buf)?;
                }
            }
            read_buf
        };
        Response::from_str(&read_buf)
    }
}

impl Display for Request {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut string = format!("{} {} HTTP/1.0\r\n", self.method, self.url.path);
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
pub struct StatusLine {
    version: String,
    status_code: u16,
    explanation: String,
}

impl StatusLine {
    fn from_str(string: &str) -> Result<Self> {
        let (version, status, explanation) = {
            let parts = string.splitn(3, ' ').collect::<Vec<_>>();
            if parts.len() < 3 {
                return Err(URLError::InvalidResponse(string.to_string()))
                    .context(format!("Can't parse status_line parts: {string}"));
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
pub struct Response {
    status_line: StatusLine,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl Response {
    pub fn from_str(string: &str) -> Result<Self> {
        let mut lines = string.lines();
        let status_line = lines
            .next()
            .ok_or_else(|| URLError::InvalidResponse(string.to_string()))
            .context("Missing status_line")?;
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

#[derive(Debug, Clone)]
pub struct URL {
    pub scheme: Scheme,
    pub path: String,
    pub host: String,
    pub port: u16,
}

impl URL {
    pub fn init(url: &str) -> Result<Self> {
        let (scheme, url) = url
            .split_once("://")
            .ok_or_else(|| URLError::Split(url.to_string()))?;
        let scheme = Scheme::from_str(scheme)?;

        let url = if url.contains('/') {
            url.to_string()
        } else {
            format!("{url}/")
        };

        let (host, url) = url
            .split_once('/')
            .ok_or_else(|| URLError::Split(url.to_string()))?;
        let path = format!("/{url}");

        let (host, port) = if let Some((new_host, port_str)) = host.split_once(':') {
            (new_host, port_str.parse::<u16>()?)
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

pub fn show(body: &str) {
    let mut in_tag = false;
    for c in body.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            print!("{c}");
        }
    }
}

pub fn load(url: &URL) -> Result<()> {
    let request = Request::init(RequestMethod::GET, url.clone());
    let response = request.make()?;
    show(
        response
            .body
            .ok_or_else(|| anyhow::anyhow!("Empty response body"))?
            .as_str(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url() {
        let url = URL::init("http://example.org").unwrap();
        assert!(matches!(url.scheme, Scheme::HTTP));
        assert_eq!(url.host, "example.org");
        assert_eq!(url.path, "/");
        assert_eq!(url.port, 80);
    }

    #[test]
    fn parse_url_https() {
        let url = URL::init("https://example.org").unwrap();
        assert!(matches!(url.scheme, Scheme::HTTPS));
        assert_eq!(url.host, "example.org");
        assert_eq!(url.path, "/");
        assert_eq!(url.port, 443);
    }

    #[test]
    fn parse_url_custom_port() {
        let url = URL::init("https://example.org:8000").unwrap();
        assert!(matches!(url.scheme, Scheme::HTTPS));
        assert_eq!(url.host, "example.org");
        assert_eq!(url.path, "/");
        assert_eq!(url.port, 8000);
    }

    #[test]
    fn load_url() {
        let url = URL::init("http://example.org").unwrap();
        load(&url).unwrap();
    }

    #[test]
    fn load_url_https() {
        let url = URL::init("https://example.org").unwrap();
        load(&url).unwrap();
    }

    // #[test]
    // fn send_connection_user_agent_headers() {
    //     let url = URL::init("https://example.org").unwrap();
    // }
}
