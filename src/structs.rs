use crate::utils::has_extension;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{collections::HashMap, ops::Add};

/// Data structure that represents a user's info
#[derive(Debug, Serialize, Deserialize)]
pub struct AboutData {
    /// Comment karma of the user
    pub comment_karma: i64,
    /// The time the user was created in seconds
    pub created: f64,
    /// The time the user was created in seconds (UTC)
    pub created_utc: f64,
    /// Undocumented
    pub has_subscribed: bool,
    /// Whether the user has verified their email
    pub has_verified_email: bool,
    /// Don't know
    pub hide_from_robots: bool,
    /// The id of the user
    pub id: String,
    /// Whether the user is a Reddit employee
    pub is_employee: bool,
    /// Whether the user is friend of the current user
    pub is_friend: bool,
    /// Whether the user has Reddit gold or not
    pub is_gold: bool,
    /// Whether the user is a moderator
    pub is_mod: bool,
    /// Link karma of the user
    pub link_karma: i64,
    /// The user's username
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserAbout {
    /// The kind of object this is. eg: Comment, Account, Subreddit, etc.
    pub kind: String,
    /// Contains data about the reddit user
    pub data: AboutData,
}

#[derive(Deserialize, Debug)]
pub struct Listing {
    /// The kind of object this is. eg: Comment, Account, Subreddit, etc.
    pub kind: String,
    /// Contains the data for the children of the listing.
    /// Listings are collections of data. For example, saved posts, hot posts in a subreddit
    pub data: ListingData,
}

#[derive(Debug)]
// When fetching directly from a post, we only care about the first listing and not the comments
// so we only deserialize the first listing. Comments break deserialization because they are missing
// a lot of fields.
pub struct SingleListing(pub Listing);

#[derive(Deserialize, Debug)]
struct ListingsFutureCompat<'a>(
    Listing,
    #[serde(default, borrow)] Option<&'a serde_json::value::RawValue>,
);

impl<'de> Deserialize<'de> for SingleListing {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let t: ListingsFutureCompat = Deserialize::deserialize(deserializer)?;
        Ok(SingleListing(t.0))
    }
}

/// The contents of a call to a 'listing' endpoint.
#[derive(Deserialize, Debug)]
pub struct ListingData {
    /// A modhash (essentially a CSRF token) generated for this request. This is generally
    /// not required for any use-case, but is provided nevertheless.
    pub modhash: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub children: Vec<Post>,
    pub dist: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Post {
    /// The kind of object this is. eg: Comment, Account, Subreddit, etc.
    pub kind: String,
    /// Contains data about this particular reddit post
    pub data: PostData,
}

/// Represents all types of link posts and self posts on Reddit.
#[derive(Deserialize, Debug, Clone)]
pub struct PostData {
    pub subreddit: String,
    /// The ID of the post in base-36 form, as used in Reddit's links.
    pub id: String,
    /// The overall points score of this post, as shown on the upvote counter. This is the
    /// same as upvotes - downvotes (however, this figure may be fuzzed by Reddit, and may not
    /// be exact)
    pub score: i64,
    /// The URL to the link thumbnail. This is "self" if this is a self post, or "default" if
    /// a thumbnail is not available.
    pub thumbnail: Option<String>,
    /// The Reddit ID for the subreddit where this was posted, **including the leading `t5_`**.
    pub subreddit_id: String,
    /// True if the logged-in user has saved this submission.
    pub saved: bool,
    /// The permanent, long link for this submission.
    pub permalink: String,
    /// The full 'Thing ID', consisting of a 'kind' and a base-36 identifier. The valid kinds are:
    /// - t1_ - Comment
    /// - t2_ - Account
    /// - t3_ - Link
    /// - t4_ - Message
    /// - t5_ - Subreddit
    /// - t6_ - Award
    /// - t8_ - PromoCampaign
    pub name: String,
    /// A timestamp of the time when the post was created, in the logged-in user's **local**
    /// time.
    pub created: Value,
    /// The linked URL, if this is a link post.
    pub url: Option<String>,
    /// The title of the post.
    pub title: Option<String>,
    /// A timestamp of the time when the post was created, in **UTC**.
    pub created_utc: Value,
    /// Media Metadata
    pub media_metadata: Option<HashMap<String, MediaMetadata>>,
    /// Gallery metadata
    pub gallery_data: Option<GalleryItems>,
    /// Is post a video?
    pub is_video: Option<bool>,
    /// Reddit Media info
    pub media: Option<PostMedia>,

    pub is_self: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MediaMetadata {
    pub status: String,
    pub e: String,
    pub m: String,
    pub id: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PostMedia {
    pub reddit_video: Option<RedditVideo>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RedditVideo {
    pub fallback_url: String,
    pub is_gif: bool,
    pub dash_url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GalleryItems {
    /// Representation containing a list of gallery items
    pub items: Vec<GalleryItem>,
}

#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct GalleryItem {
    /// The reddit media id, can be used to construct a redd.it URL
    pub media_id: String,
    /// Unique numerical ID for the specific media item
    pub id: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GfyItem {
    #[serde(rename = "gifUrl")]
    pub gif_url: String,
    #[serde(rename = "mp4Url")]
    pub mp4_url: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct StreamableApiResponse {
    pub files: HashMap<String, StreamableFile>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct StreamableFile {
    pub url: Option<String>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Summary {
    /// Number of media downloaded
    pub media_downloaded: i32,
    /// Number of media skipping downloading
    pub media_skipped: i32,
    /// Number of media supported present and parsable
    pub media_supported: i32,
}

impl Add for Summary {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            media_supported: self.media_supported + rhs.media_supported,
            media_downloaded: self.media_downloaded + rhs.media_downloaded,
            media_skipped: self.media_skipped + rhs.media_skipped,
        }
    }
}

use crate::download::*;
use log::warn;
use url::{Position, Url};
impl Post {
    pub fn get_url(&self) -> Option<String> {
        let original = self.data.url.as_ref().unwrap();
        if let Ok(mut parsed) = Url::parse(original) {
            match parsed.path_segments_mut() {
                Ok(mut p) => p.pop_if_empty(),
                Err(_) => return None,
            };
            return Some(parsed[..Position::AfterPath].to_owned());
        }
        None
    }

    pub fn get_type(&self) -> MediaType {
        if self.data.gallery_data.is_some() && self.data.media_metadata.is_some() {
            return MediaType::Gallery;
        }
        let url = match self.get_url() {
            Some(u) => u,
            None => return MediaType::Unsupported,
        };

        if url.contains(REDDIT_IMAGE_SUBDOMAIN) {
            // if the URL uses the reddit image subdomain and if the extension is
            // jpg, png or gif, then we can use the URL as is.
            if has_extension(&url, &[JPG, PNG, JPEG]) {
                return MediaType::RedditImage;
            } else if has_extension(&url, &[GIF]) {
                return MediaType::RedditGif;
            } else {
                warn!("Unsupported reddit URL: {}", url);
            }
        }
        if url.contains(REDDIT_VIDEO_SUBDOMAIN) {
            if self.data.media.is_none() {
                return MediaType::Unsupported;
            } else {
                return MediaType::RedditVideo;
            }
        }

        if url.contains(REDGIFS_DOMAIN) {
            return MediaType::RedGif;
        }
        if url.contains(GIPHY_DOMAIN) {
            return MediaType::GiphyGif;
        }
        if url.contains(IMGUR_DOMAIN) {
            if url.contains(format!("{}/a/", IMGUR_DOMAIN).as_str()) {
                return MediaType::ImgurAlbum;
            }
            if url.contains(IMGUR_SUBDOMAIN) {
                if has_extension(&url, &[GIFV, GIF, MP4]) {
                    return MediaType::ImgurGif;
                } else if has_extension(&url, &[JPG, JPEG, PNG]) {
                    return MediaType::ImgurImage;
                } else {
                    warn!("Unsupported imgur URL: {}", url);
                };
            } else {
                return MediaType::ImgurUnknown;
            }
        }
        if url.contains(STREAMABLE_DOMAIN) {
            return MediaType::StreamableVideo;
        }
        MediaType::Unsupported
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct TokenResponse {
    pub token: String,
    pub addr: String,
    pub agent: String,
    pub session: String,
    pub rtfm: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MediaInfo {
    pub sd: String,
    pub hd: String,
    // pub poster: String,
    // pub thumbnail: String,
    // pub vthumbnail: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct InnerGif {
    pub id: String,
    pub urls: MediaInfo,
}
#[derive(Deserialize, Debug, Clone)]
pub struct RedGif {
    pub gif: InnerGif,
}
