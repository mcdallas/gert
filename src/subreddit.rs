use crate::errors::GertError;
use crate::structs::{Listing, Post};
use log::{debug, error};
use reqwest::Client;
use std::fmt::Write;

pub struct Subreddit<'a> {
    /// Name of subreddit.
    pub name: String,
    url: String,
    client: &'a Client,
}

impl Subreddit<'_> {
    /// Create a new `Subreddit` instance.
    pub fn new<'a>(name: &'a str, session: &'a Client) -> Subreddit<'a> {
        let subreddit_url = format!("https://www.reddit.com/r/{}", name);

        Subreddit { name: name.to_owned(), url: subreddit_url, client: session }
    }

    async fn get_feed(
        &self,
        ty: &str,
        limit: u32,
        period: Option<&str>,
        after: Option<&str>,
    ) -> Result<Listing, GertError> {
        let url = &mut format!("{}/{}.json?limit={}", self.url, ty, limit);

        if let Some(p) = period {
            let _ = write!(url, "&t={}", p);
        }

        if let Some(a) = after {
            let _ = write!(url, "&after={}", a);
        }
        let url = &url.to_owned();
        debug!("Fetching posts from {}]", url);
        Ok(self.client.get(url).send().await.expect("Bad response").json::<Listing>().await?)
        // Ok(self.client.get(url).send().await.expect("Bad response").json::<Listing>().await.expect("Failed to parse JSON"))
    }

    pub async fn get_posts(
        &self,
        feed: &str,
        limit: u32,
        period: Option<&str>,
    ) -> Result<Vec<Post>, GertError> {
        if limit <= 100 {
            return Ok(self
                .get_feed(feed, limit, period, None)
                .await?
                .data
                .children
                .into_iter()
                .collect());
        }
        let mut page = 1;
        let mut posts: Vec<Post> = Vec::new();
        let mut after = None;
        let mut remaining = limit;
        while remaining > 0 {
            debug!("Fetching page {} of {} from r/{} [{}]", page, limit / 100, self.name, feed);
            let limit = if remaining > 100 { 100 } else { remaining };
            let listing_result = self.get_feed(feed, limit, period, after).await;

            match listing_result {
                Ok(listing) => {
                    if !listing.data.children.is_empty() {
                        posts.extend(listing.data.children.into_iter().collect::<Vec<Post>>());
                        let last_post = posts.last().unwrap();
                        after = Some(&last_post.data.name);
                        remaining -= limit;
                        page += 1;
                    } else {
                        error!("Failed to fetch posts from r/{}", self.name);
                        remaining = 0;
                    }
                }
                Err(_error) => {
                    error!("Failed to fetch posts from r/{}", self.name);
                    remaining = 0;
                }
            }
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
