use std::collections::HashMap;
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

#[derive(Debug, Clone)]
pub struct URL {
    pub scheme: Scheme,
    pub path: String,
    pub host: String,
    pub port: Option<u16>,
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

        let (mut host, url) = url
            .split_once('/')
            .ok_or_else(|| URLError::Split(url.to_string()))?;
        let path = format!("/{url}");

        let mut port = None;
        if let Some((new_host, port_str)) = host.split_once(':') {
            host = new_host;
            port = Some(port_str.parse::<u16>()?);
        }

        Ok(Self {
            scheme,
            host: host.to_string(),
            path,
            port,
        })
    }

    pub fn request(&self) -> Result<String> {
        let read_buf = {
            let mut read_buf = String::new();
            let request = format!("GET {} HTTP/1.0\r\nHost: {}\r\n\r\n", self.path, self.host);
            let port = self.port.unwrap_or(self.scheme.default_port());
            match self.scheme {
                Scheme::HTTP => {
                    let mut stream = TcpStream::connect(format!("{}:{port}", self.host))?;

                    stream.write_all(request.as_bytes())?;
                    stream.read_to_string(&mut read_buf)?;
                }
                Scheme::HTTPS => {
                    let mut client = rustls::ClientConnection::new(
                        CONFIG.clone(),
                        self.host.clone().try_into()?,
                    )?;

                    let mut socket = TcpStream::connect(format!("{}:{port}", self.host))?;
                    let mut tls = rustls::Stream::new(&mut client, &mut socket);
                    tls.write_all(request.as_bytes())?;

                    tls.read_to_string(&mut read_buf)?;
                }
            }
            read_buf
        };

        let mut lines = read_buf.lines();
        let status_line = lines
            .next()
            .ok_or_else(|| URLError::InvalidResponse(read_buf.clone()))
            .context("Can't parse status_line")?;
        let (_version, _status, _explanation) = {
            let parts = status_line.splitn(3, ' ').collect::<Vec<_>>();
            if parts.len() < 3 {
                return Err(URLError::InvalidResponse(read_buf.clone()))
                    .context(format!("Can't parse status_line parts: {status_line}",));
            }
            (parts[0], parts[1], parts[2])
        };

        let response_headers: HashMap<_, _> =
            HashMap::from_iter(lines.by_ref().map_while(|line| {
                if line == r"\r\n" {
                    None
                } else {
                    let (header, value) = line.split_once(':')?;
                    Some((header.to_lowercase(), value.trim().to_string()))
                }
            }));

        assert!(!response_headers.contains_key("transfer-encoding"));
        assert!(!response_headers.contains_key("content-encoding"));

        let content = String::from_iter(lines);
        Ok(content)
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
    let body = url.request()?;
    show(&body);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url() {
        let url = URL::init("http://example.org").unwrap();
        assert_eq!(url.scheme, "http");
        assert_eq!(url.host, "example.org");
        assert_eq!(url.path, "/");
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
}
