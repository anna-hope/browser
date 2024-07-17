use std::fmt::{Display, Formatter};
use std::num::ParseIntError;
use std::str::FromStr;
use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum UrlError {
    #[error("error splitting the URL: `{0}`")]
    Split(String),

    #[error("unknown URL scheme: {0}")]
    UnknownScheme(String),

    #[error("failed to parse the port: {0}")]
    InvalidPort(#[from] ParseIntError),

    #[error("Invalid url: {0}")]
    InvalidUrl(String),
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum Scheme {
    Http,
    Https,
    File,
    Data,
    ViewSource,
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

    fn from_str(s: &str) -> anyhow::Result<Self, Self::Err> {
        match s {
            "http" => Ok(Self::Http),
            "https" => Ok(Self::Https),
            "file" => Ok(Self::File),
            "data" => Ok(Self::Data),
            "view-source" => Ok(Self::ViewSource),
            _ => Err(UrlError::UnknownScheme(s.to_string())),
        }
    }
}

impl Display for Scheme {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let scheme_string = match self {
            // I could use some fancy PascalCase to kebab-case conversion logic,
            // but it's probably not worth it for this one-off case.
            Self::ViewSource => "view-source".to_string(),
            _ => {
                // This feels like I am abusing the Debug representation, but it works.
                format!("{self:?}")
                    .strip_prefix("Scheme::")
                    .unwrap()
                    .to_lowercase()
            }
        };
        write!(f, "{scheme_string}")
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Url {
    Web(WebUrl),
    File(FileUrl),
    Data(DataUrl),
    ViewSource(WebUrl),
}

impl Url {
    pub(crate) fn as_web_url(&self) -> Option<&WebUrl> {
        match self {
            Self::Web(url) => Some(url),
            _ => None,
        }
    }
}

impl FromStr for Url {
    type Err = UrlError;

    fn from_str(url: &str) -> anyhow::Result<Self, Self::Err> {
        let (scheme, url_rest) = url
            .split_once(':')
            .ok_or_else(|| UrlError::Split(url.to_string()))?;
        let scheme = scheme.parse::<Scheme>()?;

        if matches!(scheme, Scheme::Data) {
            return Ok(Self::Data(url_rest.parse::<DataUrl>()?));
        } else if matches!(scheme, Scheme::ViewSource) {
            // Parse the URL that view-source points to.
            return match url_rest.parse::<Self>()? {
                Self::Web(url) => Ok(Self::ViewSource(url)),
                _ => Err(UrlError::InvalidUrl(format!(
                    "Invalid resource URL for {scheme}: {}",
                    url
                ))),
            };
        }

        let url = url_rest
            .strip_prefix("//")
            .ok_or_else(|| UrlError::Split(url_rest.to_string()))?;

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
            Scheme::Data | Scheme::ViewSource => unreachable!(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WebUrl {
    pub scheme: Scheme,
    pub host: String,
    pub path: String,
    pub port: u16,
}

impl WebUrl {
    /// Convenience method to construct a new URL with the given path
    /// (useful for relative URLs, e.g. in redirects).
    pub(crate) fn with_path(&self, path: &str) -> Self {
        Self {
            scheme: self.scheme,
            host: self.host.clone(),
            path: path.to_string(),
            port: self.port,
        }
    }
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

    fn from_str(s: &str) -> anyhow::Result<Self, Self::Err> {
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    impl Url {
        fn scheme(&self) -> Scheme {
            match self {
                Self::Web(url) => url.scheme,
                Self::File(url) => url.scheme,
                Self::Data(url) => url.scheme,
                Self::ViewSource(_) => Scheme::ViewSource,
            }
        }

        fn path(&self) -> Option<&str> {
            match self {
                Self::Web(url) => Some(url.path.as_str()),
                Self::File(url) => Some(url.path.as_str()),
                _ => None,
            }
        }

        fn host(&self) -> Option<&str> {
            match self {
                Self::Web(url) => Some(url.host.as_str()),
                Self::File(url) => Some(url.host.as_str()),
                _ => None,
            }
        }

        fn port(&self) -> Option<u16> {
            match self {
                Self::Web(url) => Some(url.port),
                _ => None,
            }
        }
    }

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

    #[test]
    fn parse_view_source_url() {
        let url = "view-source:http://example.org/".parse::<Url>().unwrap();
        match url {
            Url::ViewSource(url) => {
                assert!(matches!(url.scheme, Scheme::Http));
                assert_eq!(url.host, "example.org");
                assert_eq!(url.path, "/");
                assert_eq!(url.port, 80);
            }
            _ => panic!("Expected a ViewSource url, got {url:?}"),
        }
    }
}
