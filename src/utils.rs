use crate::errors::GertError;
use log::debug;
use mime::Mime;
use reqwest::header::CONTENT_TYPE;
use std::env;
use std::path::Path;
use std::str::FromStr;
use which::which;
use xml::reader::{EventReader, XmlEvent};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Generate user agent string of the form <name>:<version>.
/// If no arguments passed generate random name and number
pub fn get_user_agent_string(username: &str) -> String {
    format!("gert:{} (by u/{})", VERSION, username)
}

/// Check if a particular path is present on the filesystem
pub fn check_path_present(file_path: &str) -> bool {
    Path::new(file_path).exists()
}

/// Function that masks sensitive data such as password and client secrets
pub fn mask_sensitive(word: &str) -> String {
    let word_length = word.len();
    return if word.is_empty() {
        // return with indication if string is empty
        String::from("<EMPTY>")
    } else if word_length > 0 && word_length <= 3 {
        // if string length is between 1-3, mask all characters
        "*".repeat(word_length)
    } else {
        // if string length greater than 5, mask all characters
        // except the first two and the last characters
        word.chars()
            .enumerate()
            .map(|(i, c)| if i == 0 || i == 1 || i == word_length - 1 { c } else { '*' })
            .collect()
    };
}

/// Check if the given application is present in the $PATH
pub fn application_present(name: String) -> bool {
    which(name).is_ok()
}

pub async fn check_url_has_mime_type(
    url: &str,
    mime_type: mime::Name<'_>,
) -> Result<bool, GertError> {
    let client = reqwest::Client::new();
    let response = client.head(url).send().await?;
    let headers = response.headers();

    match headers.get(CONTENT_TYPE) {
        None => Ok(false),
        Some(content_type) => {
            let content_type = Mime::from_str(content_type.to_str()?)?;
            let success = matches!(content_type.subtype(), _mime_type);
            debug!("Checking if URL has mime type {}, success: {}", mime_type, success);
            Ok(success)
        }
    }
}
pub struct UserEnv {
    pub username: String,
    pub password: String,
    pub client_id: String,
    pub client_secret: String,
}

pub fn parse_env_file(path: &str) -> Result<UserEnv, GertError> {
    if let Ok(env) = dotenv::from_filename(path) {
        env.load_override();
    }

    let client_id = env::var("CLIENT_ID")?;
    let client_secret = env::var("CLIENT_SECRET")?;
    let username = env::var("USERNAME")?;
    let password = env::var("PASSWORD")?;

    Ok(UserEnv { username, password, client_id, client_secret })
}

pub async fn parse_mpd(url: &str) -> (Option<String>, Option<String>) {
    
    // Parse the MPD file to get the highest quality video and audio URLs
    let response = reqwest::get(url).await.expect("Failed to fetch the URL");

    let mpd_content = response.text().await.expect("Failed to read the response");

    let parser = EventReader::from_str(&mpd_content);
    let mut max_video_bandwidth = 0;
    let mut max_audio_bandwidth = 0;
    let mut current_bandwidth = 0;
    let mut is_video = false;
    let mut max_video_url: Option<String> = None;
    let mut max_audio_url: Option<String> = None;

    for e in parser {
        match e {
            Ok(XmlEvent::StartElement { name, attributes, .. }) => {
    
                if name.local_name == "AdaptationSet" {
                    let content_type = attributes.iter().find(|attr| attr.name.local_name == "contentType");
                    match content_type {
                        Some(attr) if attr.value == "video" => {
                            is_video = true;
                        },
                        Some(attr) if attr.value == "audio" => {
                            is_video = false;
                        },
                        _ => {}
                    }
                } else if name.local_name == "Representation" {
                    current_bandwidth = attributes.iter()
                        .find(|attr| attr.name.local_name == "bandwidth")
                        .and_then(|attr| attr.value.parse().ok())
                        .unwrap_or(0);
    
                    if is_video && current_bandwidth > max_video_bandwidth {
                        max_video_bandwidth = current_bandwidth;
                    } else if !is_video && current_bandwidth > max_audio_bandwidth {
                        max_audio_bandwidth = current_bandwidth;
                    }
                }
            },
            Ok(XmlEvent::Characters(content)) => {
    
                if is_video && current_bandwidth == max_video_bandwidth {
                    max_video_url = Some(content);
                } else if !is_video && current_bandwidth == max_audio_bandwidth {
                    max_audio_url = Some(content);
                }
            },
            Err(e) => {
                println!("Error: {}", e);
                break;
            },
            _ => {}
        }
    }
    // println!("Highest quality video URL: {:?}", max_video_url);
    // println!("Highest quality audio URL: {:?}", max_audio_url);
    return (max_video_url, max_audio_url);
}