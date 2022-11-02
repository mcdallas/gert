use reqwest::Client;
use crate::errors::ReddSaverError;
use crate::structures::Listing;


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

        Subreddit {
            name: name.to_owned(),
            url: subreddit_url,
            client: Client::new(),
        }
    }

    pub async fn get_feed(
        &self,
        ty: &str,
        limit: u32,
        options: Option<&str>,
    ) -> Result<Listing, ReddSaverError> {
        let url = &mut format!("{}/{}.json?limit={}", self.url, ty, limit);

        if let Some(period) = options {
            url.push_str(&format!("&t={}", period));
        }

        Ok(self
            .client
            .get(&url.to_owned())
            .send()
            .await?
            .json::<Listing>()
            .await?)
    }

    #[allow(dead_code)]
    /// Get hot posts.
    pub async fn hot(
        &self,
        limit: u32,
        options: Option<&str>,
    ) -> Result<Listing, ReddSaverError> {
        self.get_feed("hot", limit, options).await
    }

    #[allow(dead_code)]
    /// Get rising posts.
    pub async fn rising(
        &self,
        limit: u32,
        options: Option<&str>,
    ) -> Result<Listing, ReddSaverError> {
        self.get_feed("rising", limit, options).await
    }

    #[allow(dead_code)]
    /// Get top posts.
    pub async fn top(
        &self,
        limit: u32,
        options: Option<&str>,
    ) -> Result<Listing, ReddSaverError> {
        self.get_feed("top", limit, options).await
    }

    #[allow(dead_code)]
    /// Get latest posts.
    pub async fn latest(
        &self,
        limit: u32,
        options: Option<&str>,
    ) -> Result<Listing, ReddSaverError> {
        self.get_feed("new", limit, options).await
    }


}