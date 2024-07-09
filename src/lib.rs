use anyhow;

pub(crate) struct URL {
    pub scheme: String,
    pub path: String,
    pub host: String,
}

impl URL {
    pub fn init(url: &str) -> anyhow::Result<Self> {
        let (scheme, url) = url
            .split_once("://")
            .ok_or(anyhow::anyhow!("Couldn't split {url}"))?;
        assert_eq!(scheme, "http");

        let url = if url.contains("/") {
            url.to_string()
        } else {
            format!("{url}/")
        };

        let (host, url) = url
            .split_once("/")
            .ok_or(anyhow::anyhow!("Couldn't split {url}"))?;
        let path = format!("/{url}");

        Ok(Self {
            scheme: scheme.to_string(),
            host: host.to_string(),
            path,
        })
    }
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
}
