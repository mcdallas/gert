use futures::future::join_all;
use std::borrow::Borrow;
use std::fs::File;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use std::{fs, io};
use tokio::sync::{Mutex as AsyncMutex, Semaphore};

use anyhow::{anyhow, bail, Context, Result};
use log::{debug, error, info, warn};
use url::{Position, Url};

use crate::errors::GertError;
use crate::structs::Post;
use crate::structs::{RedGif, StreamableApiResponse, TokenResponse};
use crate::utils::{check_path_present, check_url_has_mime_type, contains_any, parse_mpd};

pub static JPG: &str = "jpg";
pub static PNG: &str = "png";
pub static JPEG: &str = "jpeg";
pub const GIF: &str = "gif";
pub const GIFV: &str = "gifv";
pub const MP4: &str = "mp4";
pub const ZIP: &str = "zip";

// static REDDIT_DOMAIN: &str = "reddit.com";
pub static REDDIT_IMAGE_SUBDOMAIN: &str = "i.redd.it";
pub static REDDIT_VIDEO_SUBDOMAIN: &str = "v.redd.it";
// static REDDIT_GALLERY_PATH: &str = "gallery";

pub static IMGUR_DOMAIN: &str = "imgur.com";
pub static IMGUR_SUBDOMAIN: &str = "i.imgur.com";

pub static REDGIFS_DOMAIN: &str = "redgifs.com";
static REDGIFS_API_PREFIX: &str = "https://api.redgifs.com/v2";

pub static GIPHY_DOMAIN: &str = "giphy.com";
static GIPHY_MEDIA_SUBDOMAIN: &str = "media.giphy.com";
static GIPHY_MEDIA_SUBDOMAIN_0: &str = "media0.giphy.com";
static GIPHY_MEDIA_SUBDOMAIN_1: &str = "media1.giphy.com";
static GIPHY_MEDIA_SUBDOMAIN_2: &str = "media2.giphy.com";
static GIPHY_MEDIA_SUBDOMAIN_3: &str = "media3.giphy.com";
static GIPHY_MEDIA_SUBDOMAIN_4: &str = "media4.giphy.com";
static GIPHY_MEDIA_SUBDOMAINS: [&str; 6] = [
    GIPHY_MEDIA_SUBDOMAIN,
    GIPHY_MEDIA_SUBDOMAIN_0,
    GIPHY_MEDIA_SUBDOMAIN_1,
    GIPHY_MEDIA_SUBDOMAIN_2,
    GIPHY_MEDIA_SUBDOMAIN_3,
    GIPHY_MEDIA_SUBDOMAIN_4,
];

pub static STREAMABLE_DOMAIN: &str = "streamable.com";
static STREAMABLE_API: &str = "https://api.streamable.com/videos";

/// Media Types Supported
#[derive(Debug, PartialEq, Eq)]
pub enum MediaType {
    Gallery,
    RedditImage,
    RedditGif,
    RedditVideo,
    RedGif,
    GiphyGif,
    ImgurImage,
    ImgurGif,
    ImgurAlbum,
    ImgurUnknown,
    StreamableVideo,
    Unsupported,
}

#[derive(Debug, Clone)]
pub struct Downloader {
    posts: Vec<Post>,
    data_directory: String,
    should_download: bool,
    use_human_readable: bool,
    ffmpeg_available: bool,
    session: reqwest::Client,
    conserve_gifs: bool,
    supported: Arc<AsyncMutex<u16>>,
    skipped: Arc<AsyncMutex<u16>>,
    downloaded: Arc<AsyncMutex<u16>>,
    failed: Arc<AsyncMutex<u16>>,
    unsupported: Arc<AsyncMutex<u16>>,
    ephemeral_token: Option<String>,
}

impl Downloader {
    pub fn new(
        posts: Vec<Post>,
        data_directory: &str,
        should_download: bool,
        use_human_readable: bool,
        ffmpeg_available: bool,
        session: reqwest::Client,
        conserve_gifs: bool,
    ) -> Downloader {
        Downloader {
            posts,
            data_directory: data_directory.to_owned(),
            should_download,
            use_human_readable,
            ffmpeg_available,
            session,
            conserve_gifs,
            supported: Arc::new(AsyncMutex::new(0)),
            skipped: Arc::new(AsyncMutex::new(0)),
            downloaded: Arc::new(AsyncMutex::new(0)),
            failed: Arc::new(AsyncMutex::new(0)),
            unsupported: Arc::new(AsyncMutex::new(0)),
            ephemeral_token: None,
        }
    }

    pub async fn run(&mut self) -> Result<(), GertError> {
        let start = Instant::now();
        if self.maybe_get_redgif_token().await.is_err() {
            error!("Could not create Redgif API token.");
        }

        let downloader = Arc::new(self.clone());
        let semaphore = Arc::new(Semaphore::new(10)); // limit to 10 concurrent tasks
        let mut handles = Vec::new();
        let posts = Arc::new(std::mem::take(&mut self.posts));

        for i in 0..posts.len() {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let dl = downloader.clone();
            let posts = Arc::clone(&posts);
            let handle = tokio::spawn(async move {
                dl.process(&posts[i]).await;
                drop(permit);
            });

            handles.push(handle);
        }

        join_all(handles).await;

        let end = Instant::now();
        info!("#####################################");
        info!("Download Summary:");
        info!("Number of supported media: {}", *self.supported.lock().await);
        info!("Number of unsupported links: {}", *self.unsupported.lock().await);
        info!("Number of media downloaded: {}", *self.downloaded.lock().await);
        info!("Number of media skipped: {}", *self.skipped.lock().await);
        info!("Number of media failed to download: {}", *self.failed.lock().await);
        info!("Time taken: {:.2} seconds", (end - start).as_secs_f64());
        info!("#####################################");
        info!("FIN.");

        Ok(())
    }

    /// Generate a file name in the right format that Gert expects
    fn generate_file_name(
        &self,
        url: &str,
        subreddit: &str,
        extension: &str,
        name: &str,
        title: &str,
        index: Option<usize>,
    ) -> String {
        let idx = index.unwrap_or(0);

        return if !self.use_human_readable {
            // create a hash for the media using the URL the media is located at
            // this helps to make sure the media download always writes the same file
            // name irrespective of how many times it's run. If run more than once, the
            // media is overwritten by this method

            // Strip params from url
            let mut parsed = Url::parse(url).unwrap();
            parsed.set_query(None);
            parsed.set_fragment(None);
            let hash = md5::compute(parsed.as_str());

            if idx > 0 {
                format!("{}/{}/{:x}_{}.{}", self.data_directory, subreddit, hash, idx, extension)
            } else {
                format!("{}/{}/{:x}.{}", self.data_directory, subreddit, hash, extension)
            }
        } else {
            let disallowed_chars = [' ', '.', '/', '\\', ':', '=', '?', '"', '<', '>', '|', '*'];
            let canonical_title: String = title
                .to_lowercase()
                .chars()
                .take(200) // Truncate to avoid file system limits
                .map(|c| if disallowed_chars.contains(&c) { '_' } else { c })
                .collect();
            // create a canonical human readable file name using the post's title
            // note that the name of the post is something of the form t3_<randomstring>
            let canonical_name: String =
                if idx == 0 { String::from(name) } else { format!("{}_{}", name, idx) }
                    .replace('.', "_");
            format!(
                "{}/{}/{}_{}.{}",
                self.data_directory, subreddit, canonical_title, canonical_name, extension
            )
        };
    }

    async fn maybe_get_redgif_token(&mut self) -> Result<()> {
        let mut needs_token = false;
        if self.ephemeral_token.is_none() {
            for post in self.posts.iter() {
                if post.get_type() == MediaType::RedGif {
                    needs_token = true;
                    break;
                }
            }
        }

        if needs_token {
            let url = format!("{}/auth/temporary", REDGIFS_API_PREFIX);
            let response = self
                .session
                .get(url)
                .send()
                .await
                .context("Error contacting redgif API")?
                .json::<TokenResponse>()
                .await
                .context("Error parsing redgif API response")?;
            self.ephemeral_token = Some(response.token);
        }
        Ok(())
    }

    /// Download media from the given url and save to data directory. Also create data directory if not present already
    async fn download_media(&self, file_name: &str, url: &str) -> Result<bool, GertError> {
        // create directory if it does not already exist
        // the directory is created relative to the current working directory
        let mut status = false;
        let directory = Path::new(file_name).parent().unwrap();
        match fs::create_dir_all(directory) {
            Ok(_) => (),
            Err(_e) => return Err(GertError::CouldNotCreateDirectory),
        }

        let maybe_response = self.session.get(url).send().await;
        if let Ok(response) = maybe_response {
            // debug!("URL Response: {:#?}", response);

            let url = response.url().to_owned();
            let host_and_path = match url.host_str() {
                Some(domain) => format!("{}{}", domain, url.path()),
                None => return Err(GertError::UrlError(url::ParseError::EmptyHost)),
            };

            if host_and_path.contains("i.imgur.com/removed") {
                return Err(GertError::ImgurRemovedError);
            }

            let maybe_data = response.bytes().await;

            if let Ok(data) = maybe_data {
                debug!("Bytes length of the data: {:#?}", data.len());
                let maybe_output = File::create(file_name);
                match maybe_output {
                    Ok(mut output) => {
                        debug!("Created a file: {}", file_name);
                        match io::copy(&mut data.as_ref(), &mut output) {
                            Ok(_) => {
                                info!("Successfully saved media: {} from url {}", file_name, url);
                                status = true;
                            }
                            Err(_e) => {
                                error!("Could not save media from url {} to {}", url, file_name);
                            }
                        }
                    }
                    Err(_) => {
                        warn!("Could not create a file with the name: {}. Skipping", file_name);
                    }
                }
            }
        }

        Ok(status)
    }

    async fn process(&self, post: &Post) {
        debug!("type is : {:?}", post.get_type());
        let result = match post.get_type() {
            MediaType::Gallery => self.download_gallery(post).await,
            MediaType::RedditImage => self.download_reddit_image(post).await,
            MediaType::RedditGif => self.download_reddit_image(post).await,
            MediaType::RedditVideo => self.download_reddit_video(post).await,
            MediaType::RedGif => self.download_redgif(post).await,
            MediaType::GiphyGif => self.download_giphy(post).await,
            MediaType::ImgurGif => self.download_imgur_gif(post).await,
            MediaType::ImgurImage => self.download_imgur_image(post).await,
            MediaType::ImgurAlbum => self.download_imgur_album(post).await,
            MediaType::ImgurUnknown => self.download_imgur_unknown(post).await,
            MediaType::StreamableVideo => self.download_streamable_video(post).await,
            _ => {
                debug!("Unsupported URL: {:?}", post.get_url());
                *self.unsupported.lock().await += 1;
                Ok(())
            }
        };
        if let Err(e) = result {
            self.fail(e).await;
        }
    }

    async fn download_gallery(&self, post: &Post) -> Result<()> {
        let gallery = post.data.gallery_data.as_ref().unwrap();
        let media_metadata = post.data.media_metadata.as_ref().unwrap();

        // collect all the URLs for the images in the album
        for (index, item) in gallery.items.iter().enumerate() {
            let mut ext = JPG;
            if let Some(media) = media_metadata.get(&item.media_id) {
                ext = media.m.split('/').last().unwrap();
            }
            let url = format!("https://{}/{}.{}", REDDIT_IMAGE_SUBDOMAIN, item.media_id, ext);
            let task = DownloadTask::from_post(post, url, ext, Some(index));
            self.schedule_task(task).await;
        }
        Ok(())
    }

    async fn download_reddit_image(&self, post: &Post) -> Result<()> {
        let url = post.get_url().unwrap();
        let extension = url.split('.').last().unwrap();
        let task = DownloadTask::from_post(post, &url, extension, None);
        self.schedule_task(task).await;
        Ok(())
    }

    async fn download_redgif(&self, post: &Post) -> Result<()> {
        let url = post.get_url().unwrap();
        let id = url.split('/').last().unwrap();
        let api_url = format!("{}/gifs/{}", REDGIFS_API_PREFIX, id);
        let token = self.ephemeral_token.as_ref().context("No Redgif token found")?;
        let response = self
            .session
            .get(&api_url)
            .header("Authorization", format! {"Bearer {}", token})
            .send()
            .await
            .context("Error contacting redgif API")?
            .json::<RedGif>()
            .await
            .context(format!("Error parsing Redgif API response from {}", api_url))?;

        let task = DownloadTask::from_post(post, response.gif.urls.hd, MP4, None);
        self.schedule_task(task).await;
        Ok(())
    }

    async fn download_reddit_video(&self, post: &Post) -> Result<()> {
        let post_url = post.data.url.as_ref().unwrap();
        let extension = post_url.split('.').last().unwrap();
        let dash_url = &post.data.media.as_ref().unwrap().reddit_video.as_ref().unwrap().dash_url;

        let url = match extension {
            MP4 => {
                // if the URL uses the reddit video subdomain and if the extension is
                // mp4, then we can use the URL as is.
                post_url.to_owned()
            }
            _ => {
                // if the URL uses the reddit video subdomain, but the link does not
                // point directly to the mp4, then use the fallback URL to get the
                // appropriate link. The video quality might range from 96p to 720p
                post.data
                    .media
                    .as_ref()
                    .context("No media data found")?
                    .reddit_video
                    .as_ref()
                    .context("No fallback url found in reddit video")?
                    .fallback_url
                    .replace("?source=fallback", "")
                    .clone()
            }
        };

        let dash_video =
            url.split('/').last().context(format!("Unsupported reddit video URL: {}", url))?;

        let (maybe_video, maybe_audio) = parse_mpd(&dash_url).await;

        let mut video_url = url.clone();
        let base_path =
            &url.split('/').collect::<Vec<&str>>()[..url.split('/').count() - 1].join("/");

        if !dash_video.contains("DASH") {
            // get the video URL from the MPD file
            if maybe_video.is_none() {
                bail!("Could not find video in MPD");
            } else {
                video_url = format!("{}/{}", base_path, maybe_video.unwrap());
            }
        }

        let video_task = DownloadTask::from_post(post, video_url, MP4, None);
        let video_filename = self.schedule_task(video_task).await;

        if maybe_audio.is_some() {
            let audio_url = format!("{}/{}", base_path, maybe_audio.unwrap());
            let audio_task = DownloadTask::from_post(post, audio_url, MP4, Some(1));
            let audio_filename = self.schedule_task(audio_task).await;

            if let (Some(video_filename), Some(audio_filename)) = (video_filename, audio_filename) {
                // merge the audio and video files
                if self.stitch_audio_video(&video_filename, &audio_filename).await.is_err() {
                    debug!("Error merging audio and video files");
                }
            }
        }

        Ok(())
    }

    async fn download_giphy(&self, post: &Post) -> Result<()> {
        let url = post.data.url.as_ref().unwrap();
        let parsed = Url::parse(url).unwrap();
        let extension = url.split('.').last().unwrap();

        if contains_any(url, &GIPHY_MEDIA_SUBDOMAINS) {
            // if we encounter gif, mp4 or gifv - download as is
            match extension {
                GIF | MP4 | GIFV => {
                    let task = DownloadTask::from_post(post, url, extension, None);
                    self.schedule_task(task).await;
                }
                _ => {
                    // if the link points to the giphy post rather than the media link,
                    // use the scheme below to get the actual URL for the gif.
                    let path = &parsed[Position::AfterHost..Position::AfterPath];
                    let media_id = path.split('-').last().unwrap();
                    let giphy_url =
                        format!("https://{}/media/{}.gif", GIPHY_MEDIA_SUBDOMAIN, media_id);
                    let task = DownloadTask::from_post(post, giphy_url, GIF, None);
                    self.schedule_task(task).await;
                }
            }
        }
        Ok(())
    }

    async fn download_imgur_gif(&self, post: &Post) -> Result<()> {
        let url = post.data.url.as_ref().unwrap();

        // if the extension is gifv, then replace gifv->mp4 to get the video URL
        let task = DownloadTask::from_post(post, url.replace(".gifv", ".mp4"), MP4, None);
        self.schedule_task(task).await;
        Ok(())
    }

    async fn download_imgur_image(&self, post: &Post) -> Result<()> {
        let url = post.data.url.as_ref().unwrap();
        let extension = url.split('.').last().unwrap();

        let task = DownloadTask::from_post(post, url, extension, None);
        self.schedule_task(task).await;
        Ok(())
    }

    async fn download_imgur_unknown(&self, post: &Post) -> Result<()> {
        let url = post.data.url.as_ref().unwrap();

        // try adding the .jpg extension to the URL
        let url = format!("{}.jpg", url);
        let success = check_url_has_mime_type(&url, mime::JPEG).await.unwrap_or(false);
        if success {
            let task = DownloadTask::from_post(post, url, JPG, None);
            self.schedule_task(task).await;
            return Ok(());
        }

        let url = format!("{}.png", url);
        let success = check_url_has_mime_type(&url, mime::PNG).await.unwrap_or(false);
        if success {
            let task = DownloadTask::from_post(post, url, PNG, None);
            self.schedule_task(task).await;
            return Ok(());
        }

        bail!("Cannot determine imgur image type");
    }

    async fn download_imgur_album(&self, post: &Post) -> Result<()> {
        let url = post.data.url.as_ref().unwrap();
        let mut tokens = url.split('/').collect::<Vec<&str>>();
        tokens.push("zip");
        let url = tokens.join("/");

        let task = DownloadTask::from_post(post, url, ZIP, None);
        self.schedule_task(task).await;
        Ok(())
    }

    async fn download_streamable_video(&self, post: &Post) -> Result<()> {
        let url = post.get_url().unwrap();
        let parsed = Url::parse(&url).unwrap();
        let video_id = &parsed[Position::AfterHost..Position::AfterPath];
        let streamable_url = format!("{}{}", STREAMABLE_API, video_id);
        let response = self
            .session
            .get(&streamable_url)
            .send()
            .await
            .context("Error contacting streamable API")?;

        let parsed = response
            .json::<StreamableApiResponse>()
            .await
            .context(format!("Error parsing streamable API response from {}", streamable_url))?;

        if !parsed.files.contains_key(MP4) {
            bail!("No mp4 file found in streamable API response")
        }

        let video_url = parsed.files.get(MP4).unwrap().url.borrow().to_owned().unwrap();

        let task = DownloadTask::from_post(post, video_url, MP4, None);
        self.schedule_task(task).await;

        Ok(())
    }

    async fn fail(&self, e: anyhow::Error) {
        error!("{}", e);
        *self.failed.lock().await += 1;
    }

    async fn skip(&self, msg: &str) {
        debug!("{}", msg);
        *self.skipped.lock().await += 1;
    }

    async fn schedule_task(&self, task: DownloadTask) -> Option<String> {
        debug!("Received task: {:?}", task);
        {
            *self.supported.lock().await += 1;
        }

        if !self.should_download {
            let msg = format!("Found media at: {}", task.url);
            self.skip(&msg).await;
            return None;
        }
        let file_name = self.get_filename(&task);

        if check_path_present(&file_name)
            || check_path_present(&file_name.replace(".gif", ".mp4"))
            || check_path_present(&file_name.replace(".zip", ".jpg"))
        {
            let msg = format!("Media from url {} already downloaded. Skipping...", task.url);
            self.skip(&msg).await;
            return None;
        }

        let result = self.download_media(&file_name, &task.url).await;
        match result {
            Ok(true) => {
                {
                    *self.downloaded.lock().await += 1;
                }

                match self.post_process(file_name, &task).await {
                    Ok(filepath) => Some(filepath),
                    Err(e) => {
                        error!("Error while post processing: {}", e);
                        None
                    }
                }
            }
            Ok(false) => {
                self.fail(anyhow!("Failed to download media from url: {}", task.url)).await;
                None
            }
            Err(GertError::ImgurRemovedError) => {
                self.skip(&format!(
                    "Media from url {} has been removed from imgur. Skipping...",
                    task.url
                ))
                .await;
                None
            }
            Err(e) => {
                self.fail(anyhow!("Error while downloading media from url {}: {}", task.url, e))
                    .await;
                None
            }
        }
    }

    async fn post_process(
        &self,
        download_path: String,
        task: &DownloadTask,
    ) -> Result<String, GertError> {
        if !self.ffmpeg_available {
            return Ok(download_path);
        };

        if task.extension == GIF && !self.conserve_gifs {
            //If ffmpeg is installed convert gifs to mp4
            let output_file = download_path.replace(".gif", ".mp4");
            if check_path_present(&output_file) {
                return Ok(output_file);
            }
            debug!("Converting gif to mp4: {}", output_file);
            let mut command = tokio::process::Command::new("ffmpeg")
                .arg("-i")
                .arg(&download_path)
                .arg("-movflags")
                .arg("+faststart")
                .arg("-pix_fmt")
                .arg("yuv420p")
                .arg("-vf")
                .arg("scale=trunc(iw/2)*2:trunc(ih/2)*2")
                .arg(&output_file)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;

            let status = command.wait().await?;
            if status.success() {
                // Cleanup the gif
                fs::remove_file(download_path)?;
                return Ok(output_file);
            } else {
                return Err(GertError::FfmpegError("Failed to convert gif to mp4".into()));
            }
        }
        if task.extension == ZIP {
            let file = File::open(&download_path)?;
            let mut archive = zip::ZipArchive::new(file)?;

            for i in 0..archive.len() {
                // Unzip the contents of the zip file

                let mut file = archive.by_index(i)?;
                let extension = file.name().split('.').last().unwrap();

                let filename = self.generate_file_name(
                    &task.url,
                    &task.subreddit,
                    extension,
                    &task.post_name,
                    &task.post_title,
                    Some(i),
                );
                debug!("Unzipping file: {}", filename);
                let mut outfile = fs::File::create(filename)?;
                io::copy(&mut file, &mut outfile)?;
            }
            // Cleanup the zip
            fs::remove_file(&download_path)?;
        }

        Ok(download_path)
    }

    async fn stitch_audio_video(
        &self,
        video_path: &str,
        audio_path: &str,
    ) -> Result<String, GertError> {
        let output_file = video_path.replace(".mp4", "-merged.mp4");
        let mut command = tokio::process::Command::new("ffmpeg")
            .arg("-i")
            .arg(video_path)
            .arg("-i")
            .arg(audio_path)
            .arg("-c")
            .arg("copy")
            .arg("-map")
            .arg("1:a")
            .arg("-map")
            .arg("0:v")
            .arg(&output_file)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let status = command.wait().await?;
        if status.success() {
            // Cleanup the single streams
            fs::remove_file(video_path)?;
            fs::remove_file(audio_path)?;

            fs::rename(output_file, video_path)?;
            debug!("Successfully merged audio and video: {}", video_path);
            return Ok(video_path.to_owned());
        } else {
            fs::remove_file(audio_path)?;
            return Err(GertError::FfmpegError("Failed to merge audio and video".into()));
        }
    }

    fn get_filename(&self, task: &DownloadTask) -> String {
        self.generate_file_name(
            &task.url,
            &task.subreddit,
            &task.extension,
            &task.post_name,
            &task.post_title,
            task.index,
        )
    }
}
#[derive(Debug)]
struct DownloadTask {
    url: String,
    subreddit: String,
    extension: String,
    post_name: String,
    post_title: String,
    index: Option<usize>,
}
impl DownloadTask {
    fn from_post<U: Into<String>, V: Into<String>>(
        post: &Post,
        url: U,
        extension: V,
        index: Option<usize>,
    ) -> DownloadTask {
        DownloadTask {
            url: url.into(),
            subreddit: post.data.subreddit.to_owned(),
            extension: extension.into(),
            post_name: post.data.name.to_owned(),
            post_title: post.data.title.clone().unwrap(),
            index,
        }
    }
}
