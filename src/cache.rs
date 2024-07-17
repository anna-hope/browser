use std::collections::HashMap;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::{DateTime, FixedOffset, Local, TimeDelta};

use crate::request::Response;
use crate::url::WebUrl;

#[derive(Debug)]
struct ResponseCacheProperties {
    date: DateTime<FixedOffset>,
    // Store as TimeDelta instead of Duration to catch out-of-bounds errors at creation
    // and reduce overhead later when comparing with current time.
    max_age: TimeDelta,
}

impl ResponseCacheProperties {
    fn from_response(response: &Response) -> Result<Self> {
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
            return Err(anyhow!("No cache-control header in headers: {headers:?}"));
        };

        Ok(Self { date, max_age })
    }
}

#[derive(Debug)]
struct ResponseWithCacheProperties {
    response: Response,
    cache_properties: ResponseCacheProperties,
}

impl ResponseWithCacheProperties {
    fn new(response: Response) -> Result<Self> {
        let cache_properties = ResponseCacheProperties::from_response(&response)?;
        Ok(Self {
            response,
            cache_properties,
        })
    }
}

#[derive(Debug, Default)]
pub struct Cache {
    cache: HashMap<WebUrl, ResponseWithCacheProperties>,
}

impl Cache {
    fn remove(&mut self, url: &WebUrl) -> Option<Response> {
        self.cache.remove(url).map(|r| r.response)
    }

    pub fn get(&self, url: &WebUrl) -> Option<&Response> {
        if let Some(response_with_cache_props) = self.cache.get(url) {
            let current_time = Local::now().fixed_offset();
            let delta = current_time - response_with_cache_props.cache_properties.date;
            if delta < response_with_cache_props.cache_properties.max_age {
                Some(&response_with_cache_props.response)
            } else {
                // Evict this response.
                None
            }
        } else {
            None
        }
    }

    pub fn insert(&mut self, url: WebUrl, response: Response) -> Result<()> {
        let response_with_cache_properties = ResponseWithCacheProperties::new(response)?;
        self.cache.insert(url, response_with_cache_properties);
        Ok(())
    }
}
