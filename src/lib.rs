use std::collections::HashMap;
use std::io;
use std::io::{Read, Write};
use std::net::TcpStream;

use anyhow::{Context, Result};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum URLError {
    #[error("error splitting the URL: `{0}`")]
    Split(String),

    #[error("can't connect via TCP")]
    ConnectionFailed(#[from] io::Error),

    #[error("invalid TCP response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug)]
pub struct URL {
    pub scheme: String,
    pub path: String,
    pub host: String,
}

impl URL {
    pub fn init(url: &str) -> Result<Self> {
        let (scheme, url) = url
            .split_once("://")
            .ok_or_else(|| URLError::Split(url.to_string()))?;
        assert_eq!(scheme, "http");

        let url = if url.contains("/") {
            url.to_string()
        } else {
            format!("{url}/")
        };

        let (host, url) = url
            .split_once("/")
            .ok_or_else(|| URLError::Split(url.to_string()))?;
        let path = format!("/{url}");

        Ok(Self {
            scheme: scheme.to_string(),
            host: host.to_string(),
            path,
        })
    }

    pub fn request(&self) -> Result<String> {
        let mut stream = TcpStream::connect(format!("{}:80", self.host))?;
        let request = format!("GET {} HTTP/1.0\r\nHost: {}\r\n\r\n", self.path, self.host);

        stream.write_all(request.as_bytes())?;
        let mut read_buf = String::new();
        stream.read_to_string(&mut read_buf)?;
        let mut lines = read_buf.lines();
        let status_line = lines
            .next()
            .ok_or_else(|| URLError::InvalidResponse(read_buf.clone()))
            .context("Can't parse status_line")?;
        let (_version, _status, _explanation) = {
            let parts = status_line.splitn(3, " ").collect::<Vec<_>>();
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
                    let (header, value) = line.split_once(":")?;
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
}
