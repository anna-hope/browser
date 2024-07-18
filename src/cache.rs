use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::{DateTime, FixedOffset, Local, TimeDelta};

use crate::request::Response;
use crate::url::WebUrl;

type ResponseCacheProperties = (DateTime<FixedOffset>, TimeDelta);

#[derive(Debug, PartialEq)]
struct ResponseWithCacheProperties {
    response: Arc<Response>,
    date: DateTime<FixedOffset>,
    // Store as TimeDelta instead of Duration to avoid recomputing it and handling potential
    // errors every time we query the cache.
    max_age: TimeDelta,
}

impl ResponseWithCacheProperties {
    fn parse_cache_properties(response: &Response) -> Result<ResponseCacheProperties> {
        let headers = &response.headers;
        let date = headers
            .get("date")
            .ok_or_else(|| anyhow!("Missing date in headers"))
            .map(|s| DateTime::parse_from_rfc2822(s.as_str()))??;

        let max_age = if let Some(cache_control) = headers.get("cache-control") {
            let max_age = cache_control
                .strip_prefix("max-age=")
                .ok_or_else(|| anyhow!("Invalid value for cache-control: {cache_control}"))?;
            let max_age = max_age.parse::<u64>().map(Duration::from_secs)?;

            TimeDelta::from_std(max_age)?
        } else {
            return Err(anyhow!("No cache-control header: {headers:?}"));
        };

        Ok((date, max_age))
    }

    fn new(response: Response) -> Result<Self> {
        let (date, max_age) = Self::parse_cache_properties(&response)?;
        Ok(Self {
            response: Arc::new(response),
            date,
            max_age,
        })
    }
}

#[derive(Default)]
pub(crate) struct MaybeCachedResponse {
    inner: Option<Weak<Response>>,
}

impl MaybeCachedResponse {
    fn new(wrapped_response: &Arc<Response>) -> Self {
        Self {
            inner: Some(Arc::downgrade(wrapped_response)),
        }
    }

    pub(crate) fn get(&self) -> Option<impl AsRef<Response>> {
        self.inner.as_ref().map(Weak::upgrade)?
    }
}

#[derive(Debug, Default)]
pub struct Cache {
    cache: HashMap<WebUrl, ResponseWithCacheProperties>,
}

impl Cache {
    pub fn insert(&mut self, url: WebUrl, response: Response) -> Result<()> {
        let response_with_cache_properties = ResponseWithCacheProperties::new(response)?;
        self.cache.insert(url, response_with_cache_properties);
        Ok(())
    }

    fn remove(&mut self, url: &WebUrl) -> Option<Response> {
        self.cache
            .remove(url)
            .map(|r| Arc::unwrap_or_clone(r.response))
    }

    pub fn get(&self, url: &WebUrl) -> MaybeCachedResponse {
        if let Some(response_with_cache_props) = self.cache.get(url) {
            let current_time = Local::now().fixed_offset();
            let delta = current_time - response_with_cache_props.date;
            if delta < response_with_cache_props.max_age {
                return MaybeCachedResponse::new(&response_with_cache_props.response);
            }
        }

        MaybeCachedResponse::default()
    }
}
