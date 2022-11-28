use crate::errors::GertError;
use crate::structures::{Listing, Post};
use reqwest::Client;
use std::fmt::Write;
use log::debug;

pub struct Subreddit {
    /// Name of subreddit.
    pub name: String,
    url: String,
    client: Client,
}

impl Subreddit {
    /// Create a new `Subreddit` instance.
    pub fn new(name: &str) -> Subreddit {
        let subreddit_url = format!("https://www.reddit.com/r/{}", name);

        Subreddit { name: name.to_owned(), url: subreddit_url, client: Client::new() }
    }

    async fn get_feed(
        &self,
        ty: &str,
        limit: u32,
        period: Option<&str>,
        after: Option<&str>
    ) -> Result<Listing, GertError> {
        let url = &mut format!("{}/{}.json?limit={}", self.url, ty, limit);

        if let Some(p) = period {
            let _ = write!(url, "&t={}", p);
        }

        if let Some(a) = after {
            let _ = write!(url, "&after={}", a);
        }

        Ok(self.client.get(&url.to_owned()).send().await?.json::<Listing>().await?)
    }

    pub async fn get_posts(&self, feed:&str, limit: u32, period: Option<&str>) -> Result<Vec<Post>, GertError> {
        if limit <= 100 {
            return Ok(self.get_feed(feed, limit, period, None).await?.data
            .children
            .into_iter().collect())
        }
        let mut page = 1;
        let mut posts: Vec<Post> = Vec::new();
        let mut after = None;
        let mut remaining = limit;
        while remaining > 0 {
            debug!("Fetching page {} of {} from r/{} [{}]", page, limit / 100, self.name, feed);
            let limit = if remaining > 100 { 100 } else { remaining };
            let listing = self.get_feed(feed, limit, period, after).await?;
            
            posts.extend(listing.data.children.into_iter().collect::<Vec<Post>>());
            let last_post = posts.last().unwrap();
            after = Some(&last_post.data.name);
            remaining -= limit;
            page+=1;
        }
        Ok(posts)
    }

    #[allow(dead_code)]
    /// Get hot posts.
    pub async fn hot(&self, limit: u32, options: Option<&str>) -> Result<Listing, GertError> {
        self.get_feed("hot", limit, options, None).await
    }

    #[allow(dead_code)]
    /// Get rising posts.
    pub async fn rising(&self, limit: u32, period: Option<&str>) -> Result<Listing, GertError> {
        self.get_feed("rising", limit, period, None).await
    }

    #[allow(dead_code)]
    /// Get top posts.
    pub async fn top(&self, limit: u32, period: Option<&str>) -> Result<Listing, GertError> {
        self.get_feed("top", limit, period, None).await
    }

    #[allow(dead_code)]
    /// Get latest posts.
    pub async fn latest(&self, limit: u32, period: Option<&str>) -> Result<Listing, GertError> {
        self.get_feed("new", limit, period, None).await
    }
}
