use std::fmt::{Display, Formatter};
use std::num::ParseIntError;
use std::str::FromStr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UrlError {
    #[error("error splitting the URL: `{0}`")]
    Split(String),

    #[error("unknown URL scheme: {0}")]
    UnknownScheme(String),

    #[error("failed to parse the port: {0}")]
    InvalidPort(#[from] ParseIntError),

    #[error("Invalid url: {0}")]
    InvalidUrl(String),
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum Scheme {
    Http,
    Https,
    File,
    Data,
    ViewSource,
    About,
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
            "about" => Ok(Self::About),
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
                format!("{self:?}").to_lowercase()
            }
        };
        write!(f, "{scheme_string}")
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub enum AboutValue {
    #[default]
    Blank,
}

impl FromStr for AboutValue {
    type Err = UrlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "blank" => Ok(Self::Blank),
            _ => Err(UrlError::InvalidUrl(s.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Url {
    Web(WebUrl),
    File(FileUrl),
    Data(DataUrl),
    ViewSource(WebUrl),
    About(AboutValue),
}

impl Url {
    pub fn as_web_url(&self) -> Option<&WebUrl> {
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
        } else if matches!(scheme, Scheme::About) {
            let about_value = url_rest.parse::<AboutValue>()?;
            return Ok(Self::About(about_value));
        };

        let url = url_rest
            .strip_prefix("//")
            .ok_or_else(|| UrlError::Split(url_rest.to_string()))?;

        let url = if url.contains('/') {
            url.to_string()
        } else {
            format!("{url}/")
        };

        // We are guaranteed to have a / in the URL now, so safe to unwrap.
        #[allow(clippy::unwrap_used)]
        let (host, url) = url.split_once('/').unwrap();
        let path = format!("/{url}");

        match scheme {
            Scheme::Http | Scheme::Https => {
                let (host, port) = if let Some((new_host, port_str)) = host.split_once(':') {
                    (new_host, port_str.parse::<u16>()?)
                } else {
                    // Http and Https are guaranteed to have a default port, so safe to unwrap.
                    #[allow(clippy::unwrap_used)]
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
            Scheme::Data | Scheme::ViewSource | Scheme::About => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct WebUrl {
    pub scheme: Scheme,
    pub host: String,
    pub path: String,
    pub port: u16,
}

impl WebUrl {
    /// Convenience method to construct a new URL with the given path
    /// (useful for relative URLs, e.g. in redirects).
    pub fn with_path(&self, path: &str) -> Self {
        Self {
            scheme: self.scheme,
            host: self.host.clone(),
            path: path.to_string(),
            port: self.port,
        }
    }
}

impl Display for WebUrl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}://{}:{}{}",
            self.scheme, self.host, self.port, self.path
        )
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FileUrl {
    pub scheme: Scheme,
    pub path: String,
    pub host: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DataUrl {
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
mod tests {
    use super::*;
    use anyhow::{anyhow, Result};

    impl Url {
        fn scheme(&self) -> Scheme {
            match self {
                Self::Web(url) => url.scheme,
                Self::File(url) => url.scheme,
                Self::Data(url) => url.scheme,
                Self::ViewSource(_) => Scheme::ViewSource,
                Self::About(_) => Scheme::About,
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
    fn parse_url() -> Result<()> {
        let url = "http://example.org".parse::<Url>()?;
        assert!(matches!(url.scheme(), Scheme::Http));
        assert_eq!(url.host(), Some("example.org"));
        assert_eq!(url.path(), Some("/"));
        assert_eq!(url.port(), Some(80));
        Ok(())
    }

    #[test]
    fn parse_url_https() -> Result<()> {
        let url = "https://example.org".parse::<Url>()?;
        assert!(matches!(url.scheme(), Scheme::Https));
        assert_eq!(url.host(), Some("example.org"));
        assert_eq!(url.path(), Some("/"));
        assert_eq!(url.port(), Some(443));
        Ok(())
    }

    #[test]
    fn parse_url_custom_port() -> Result<()> {
        let url = "https://example.org:8000".parse::<Url>()?;
        assert!(matches!(url.scheme(), Scheme::Https));
        assert_eq!(url.host(), Some("example.org"));
        assert_eq!(url.path(), Some("/"));
        assert_eq!(url.port(), Some(8000));
        Ok(())
    }

    #[test]
    fn parse_data_url() -> Result<()> {
        let url = "data:text/html,Hello world!".parse::<Url>()?;
        match url {
            Url::Data(url) => {
                assert!(matches!(url.scheme, Scheme::Data));
                assert_eq!(url.mimetype, "text/html");
                assert_eq!(url.data, "Hello world!");
            }
            _ => return Err(anyhow!("Expected a DataUrl, got {url:?}")),
        }
        Ok(())
    }

    #[test]
    fn parse_view_source_url() -> Result<()> {
        let url = "view-source:http://example.org/".parse::<Url>()?;
        match url {
            Url::ViewSource(url) => {
                assert!(matches!(url.scheme, Scheme::Http));
                assert_eq!(url.host, "example.org");
                assert_eq!(url.path, "/");
                assert_eq!(url.port, 80);
            }
            _ => return Err(anyhow!("Expected a ViewSource url, got {url:?}")),
        }
        Ok(())
    }

    #[test]
    fn web_url_display() -> Result<()> {
        let url_str = "https://example.org:443/";
        let url = url_str.parse::<Url>()?;
        #[allow(clippy::unwrap_used)]
        let web_url = url.as_web_url().unwrap();
        assert_eq!(web_url.to_string().as_str(), url_str);
        Ok(())
    }

    #[test]
    fn web_url_display_2() -> Result<()> {
        let url_str = "https://browser.engineering/http.html";
        let url = url_str.parse::<Url>()?;
        #[allow(clippy::unwrap_used)]
        let web_url = url.as_web_url().unwrap();
        assert_eq!(
            web_url.to_string().as_str(),
            "https://browser.engineering:443/http.html"
        );
        Ok(())
    }

    #[test]
    fn about_blank() -> Result<()> {
        let url = "about:blank";
        let url = url.parse::<Url>()?;
        assert!(matches!(url, Url::About(AboutValue::Blank)));
        Ok(())
    }
}
