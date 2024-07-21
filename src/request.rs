use std::cell::OnceCell;
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

use crate::headers::{Headers, HeadersError, USER_AGENT};
use crate::url::{Scheme, UrlError, WebUrl};

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

    #[allow(clippy::unwrap_used)]
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

#[derive(Error, Debug)]
pub(crate) enum RequestError {
    #[error("invalid scheme for a web URL: {0}")]
    InvalidScheme(Scheme),

    #[error("can't connect via TCP")]
    ConnectionFailed(#[from] io::Error),
}

#[derive(Debug)]
pub(crate) struct Request {
    method: RequestMethod,
    headers: Headers,
    stream: ReusableTcpStream,
}

impl Request {
    pub(crate) fn new(method: RequestMethod, host: &str, keep_alive: bool, gzip: bool) -> Self {
        let connection_value = if keep_alive { "keep-alive" } else { "close" };
        let mut headers = Headers::from(&[("Host", &[host]), ("Connection", &[connection_value])]);

        if gzip {
            headers.add("Accept-Encoding", "gzip")
        }

        Self {
            method,
            headers,
            stream: ReusableTcpStream::new(),
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
        // TODO: add body
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

        Ok(Response::from_stream(stream)?)
    }
}

impl Request {
    /// Convenience method to make a GET request
    /// to the given URL with the default `User-Agent`,
    /// and return the resulting `Response` or error.
    pub(crate) fn get(url: &WebUrl) -> Result<Response> {
        let mut request = Self::new(RequestMethod::Get, &url.host, false, true)
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
fn decompress_gzip(bytes: impl Read) -> Result<String, ResponseError> {
    let mut gz = GzDecoder::new(bytes);
    let mut string = String::new();
    gz.read_to_string(&mut string)?;
    Ok(string)
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
    ParseHeaders(String),

    #[error("invalid headers: {0}")]
    InvalidHeaders(#[from] HeadersError),

    #[error("error reading the response stream: {0}")]
    Stream(#[from] io::Error),
}

#[inline]
fn read_chunked(reader: &mut BufReader<&mut impl Read>) -> Result<Vec<u8>, ResponseError> {
    let mut current_chunk_len_line = String::new();
    let mut response_body = vec![];
    loop {
        // Read the chunk length.
        reader.read_line(&mut current_chunk_len_line)?;
        let chunk_len = usize::from_str_radix(current_chunk_len_line.trim(), 16)?;

        if chunk_len > 0 {
            let mut chunk_buf = vec![0; chunk_len];
            reader.read_exact(&mut chunk_buf)?;
            response_body.append(&mut chunk_buf);
            // Skip the newline at the end
            reader.read_line(&mut current_chunk_len_line)?;
        } else {
            break;
        }
        current_chunk_len_line.clear();
    }
    Ok(response_body)
}

#[inline]
fn read_body(
    reader: &mut BufReader<&mut impl Read>,
    headers: &Headers,
) -> Result<Option<String>, ResponseError> {
    let buf = if headers.has_given_value("transfer-encoding", "chunked") == Some(true) {
        read_chunked(reader)?
    } else {
        // The two calls to transpose here are a bit awkward, but they help us deal
        // with the whole Option<Result> thing and make sure
        // we handle the errors from both not having a content-length header at all,
        // and not having a valid value for the content-length.
        let content_length = headers
            .get_single_value("content-length")
            .transpose()?
            .map(|s| s.parse::<usize>())
            .transpose()?
            .unwrap_or(0);

        let mut buf = vec![0u8; content_length];
        reader.read_exact(&mut buf)?;
        // let mut bytes_read = 0;
        // while bytes_read < content_length {
        //     let new_bytes_read = reader.read(&mut buf)?;
        //     if new_bytes_read == 0 {
        //         if !reader.buffer().is_empty() {
        //             eprintln!(
        //                 "Got no new bytes, but buffer still has {} bytes left",
        //                 reader.buffer().len()
        //             );
        //         }
        //         break;
        //     }
        //     bytes_read += new_bytes_read;
        // }

        buf
    };

    if !buf.is_empty() {
        let body = if headers.has_given_value("content-encoding", "gzip") == Some(true) {
            decompress_gzip(buf.as_slice())?
        } else {
            String::from_utf8_lossy(&buf).to_string()
        };
        Ok(Some(body))
    } else {
        Ok(None)
    }
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

        let mut current_line = String::new();
        let mut headers = Headers::default();

        loop {
            reader.read_line(&mut current_line)?;
            if current_line.as_str() == "\r\n" {
                break;
            }

            let (header, values) = current_line
                .split_once(':')
                .ok_or_else(|| ResponseError::ParseHeaders(current_line.clone()))?;
            headers.add(header, values.trim());
            current_line.clear();
        }

        let body = read_body(&mut reader, &headers)?;

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
    fn close() -> Result<()> {
        let url = "http://example.com".parse::<Url>()?;
        #[allow(clippy::unwrap_used)]
        let response = Request::get(url.as_web_url().unwrap())?;
        assert!(response.body.is_some());

        let url = "https://browser.engineering/http.html".parse::<Url>()?;
        #[allow(clippy::unwrap_used)]
        let response = Request::get(url.as_web_url().unwrap())?;
        assert!(response.body.is_some());
        Ok(())
    }

    fn test_url_keepalive(url: &str) -> Result<()> {
        let url = url.parse::<Url>()?;
        #[allow(clippy::unwrap_used)]
        let url = url.as_web_url().unwrap();

        let mut request = Request::new(RequestMethod::Get, &url.host, true, true);
        let first_response = request.make(url, None)?;
        assert!(first_response.body.is_some());
        let second_response = request.make(url, None)?;
        assert_eq!(first_response, second_response);

        let one_off_response = Request::get(url)?;
        assert_eq!(first_response.body, one_off_response.body);
        assert_eq!(second_response.body, one_off_response.body);
        Ok(())
    }

    #[test]
    fn keep_alive() -> Result<()> {
        test_url_keepalive("http://example.com")?;
        test_url_keepalive("http://browser.engineering/http.html")
    }

    #[test]
    fn keep_alive_https() -> Result<()> {
        test_url_keepalive("https://example.com")?;
        test_url_keepalive("https://browser.engineering/http.html")
    }

    #[test]
    fn gzipped_matches_uncompressed() -> Result<()> {
        let url = "https://browser.engineering/http.html".parse::<Url>()?;
        #[allow(clippy::unwrap_used)]
        let url = url.as_web_url().unwrap();

        let mut request_uncompressed =
            Request::new(RequestMethod::Get, url.host.as_str(), true, false);
        let response_uncompressed = request_uncompressed.make(url, None)?;

        let mut request_compressed =
            Request::new(RequestMethod::Get, url.host.as_str(), true, true);
        let response_compressed = request_compressed.make(url, None)?;

        assert_eq!(response_compressed.body, response_uncompressed.body);
        Ok(())
    }
}
