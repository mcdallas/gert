use std::borrow::Borrow;
use std::fs::File;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::{fs, io};

use anyhow::{anyhow, bail, Context, Result};
use log::{debug, error, info, warn};
use reqwest::StatusCode;
use serde_json::json;
use url::{Position, Url};

use crate::errors::GertError;
use crate::structures::Post;
use crate::structures::{GfyData, StreamableApiResponse};
use crate::utils::{check_path_present, check_url_has_mime_type};

pub static JPG_EXTENSION: &str = "jpg";
pub static PNG_EXTENSION: &str = "png";
pub static JPEG_EXTENSION: &str = "jpeg";
pub const GIF_EXTENSION: &str = "gif";
pub const GIFV_EXTENSION: &str = "gifv";
const MP4_EXTENSION: &str = "mp4";
const ZIP_EXTENSION: &str = "zip";

// static REDDIT_DOMAIN: &str = "reddit.com";
pub static REDDIT_IMAGE_SUBDOMAIN: &str = "i.redd.it";
pub static REDDIT_VIDEO_SUBDOMAIN: &str = "v.redd.it";
// static REDDIT_GALLERY_PATH: &str = "gallery";

pub static IMGUR_DOMAIN: &str = "imgur.com";
pub static IMGUR_SUBDOMAIN: &str = "i.imgur.com";

pub static GFYCAT_DOMAIN: &str = "gfycat.com";
static GFYCAT_API_PREFIX: &str = "https://api.gfycat.com/v1/gfycats";

pub static REDGIFS_DOMAIN: &str = "redgifs.com";
static REDGIFS_API_PREFIX: &str = "https://api.redgifs.com/v1/gfycats";

pub static GIPHY_DOMAIN: &str = "giphy.com";
static GIPHY_MEDIA_SUBDOMAIN: &str = "media.giphy.com";
static GIPHY_MEDIA_SUBDOMAIN_0: &str = "media0.giphy.com";
static GIPHY_MEDIA_SUBDOMAIN_1: &str = "media1.giphy.com";
static GIPHY_MEDIA_SUBDOMAIN_2: &str = "media2.giphy.com";
static GIPHY_MEDIA_SUBDOMAIN_3: &str = "media3.giphy.com";
static GIPHY_MEDIA_SUBDOMAIN_4: &str = "media4.giphy.com";

pub static STREAMABLE_DOMAIN: &str = "streamable.com";
static STREAMABLE_API: &str = "https://api.streamable.com/videos";

/// Media Types Supported
#[derive(Debug, PartialEq, Eq)]
pub enum MediaType {
    Gallery,
    RedditImage,
    RedditGif,
    RedditVideo,
    GfycatGif,
    GiphyGif,
    ImgurImage,
    ImgurGif,
    ImgurAlbum,
    ImgurUnknown,
    StreamableVideo,
    Unsupported,
}

#[derive(Debug)]
pub struct Downloader<'a> {
    posts: Vec<Post>,
    post_json: serde_json::Value,
    export_json: bool,
    data_directory: &'a str,
    should_download: bool,
    use_human_readable: bool,
    ffmpeg_available: bool,
    session: &'a reqwest::Client,
    supported: Arc<Mutex<u16>>,
    skipped: Arc<Mutex<u16>>,
    downloaded: Arc<Mutex<u16>>,
    failed: Arc<Mutex<u16>>,
    unsupported: Arc<Mutex<u16>>,
}

impl<'a> Downloader<'a> {
    pub fn new(
        posts: Vec<Post>,
        data_directory: &'a str,
        should_download: bool,
        export_json: bool,
        use_human_readable: bool,
        ffmpeg_available: bool,
        session: &'a reqwest::Client,
    ) -> Downloader<'a> {
        Downloader {
            posts,
            post_json: json!({}),
            data_directory,
            should_download,
            export_json,
            use_human_readable,
            ffmpeg_available,
            session,
            supported: Arc::new(Mutex::new(0)),
            skipped: Arc::new(Mutex::new(0)),
            downloaded: Arc::new(Mutex::new(0)),
            failed: Arc::new(Mutex::new(0)),
            unsupported: Arc::new(Mutex::new(0)),
        }
    }

    // Downloads the media from the given URL and then returns the JSON data
    // about the media
    pub async fn run(&mut self) -> Result<serde_json::Value, GertError> {
        for post in self.posts.clone().iter() {
            self.process(post).await;
        }

        info!("#####################################");
        info!("Download Summary:");
        info!("Number of supported media: {}", *self.supported.lock().unwrap());
        info!("Number of unsupported links: {}", *self.unsupported.lock().unwrap());
        info!("Number of media downloaded: {}", *self.downloaded.lock().unwrap());
        info!("Number of media skipped: {}", *self.skipped.lock().unwrap());
        info!("Number of media failed to download: {}", *self.failed.lock().unwrap());
        info!("#####################################");
        info!("FIN.");

        Ok(self.post_json.clone())
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
            let canonical_title: String = title
                .to_lowercase()
                .chars()
                // to make sure file names don't exceed operating system maximums, truncate at 200
                // you could possibly stretch beyond 200, but this is a conservative estimate that
                // leaves 55 bytes for the name string
                .take(200)
                .enumerate()
                .map(|(_, c)| {
                    if c.is_whitespace()
                        || c == '.'
                        || c == '/'
                        || c == '\\'
                        || c == ':'
                        || c == '='
                    {
                        '_'
                    } else {
                        c
                    }
                })
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

    /// Download media from the given url and save to data directory. Also create data directory if not present already
    async fn download_media(&mut self, file_name: &str, url: &str, post: &Post) -> Result<bool, GertError> {
        // create directory if it does not already exist
        // the directory is created relative to the current working directory
        let mut status = false;
        let directory = Path::new(file_name).parent().unwrap();
        fs::create_dir_all(directory)?;

        let response = self.session.get(url).send().await?;
        // debug!("URL Response: {:#?}", response);

        let data = response.bytes().await?;
        debug!("Bytes length of the data: {:#?}", data.len());

        let maybe_output = File::create(file_name);
        match maybe_output {
            Ok(mut output) => {
                debug!("Created a file: {}", file_name);
                match io::copy(&mut data.as_ref(), &mut output) {
                    Ok(_) => {
                        info!("Successfully saved media: {} from url {}", file_name, url);
                        status = true;

                        if self.export_json {
                            self.post_json[file_name] = serde_json::to_value(post)?;
                            self.post_json[file_name]["success"] = serde_json::to_value(true)?;
                        }
                    }
                    Err(_e) => {
                        error!("Could not save media from url {} to {}", url, file_name);

                        if self.export_json {
                            self.post_json[file_name] = serde_json::to_value(post)?;
                            self.post_json[file_name]["success"] = serde_json::to_value(false)?;
                        }
                    }
                }
            }
            Err(_) => {
                warn!("Could not create a file with the name: {}. Skipping", file_name);
            }
        }

        Ok(status)
    }

    async fn process(&mut self, post: &Post) {
        debug!("type is : {:?}", post.get_type());
        let result = match post.get_type() {
            MediaType::Gallery => self.download_gallery(post).await,
            MediaType::RedditImage => self.download_reddit_image(post).await,
            MediaType::RedditGif => self.download_reddit_image(post).await,
            MediaType::RedditVideo => self.download_reddit_video(post).await,
            MediaType::GfycatGif => self.download_gfycat(post).await,
            MediaType::GiphyGif => self.download_giphy(post).await,
            MediaType::ImgurGif => self.download_imgur_gif(post).await,
            MediaType::ImgurImage => self.download_imgur_image(post).await,
            MediaType::ImgurAlbum => self.download_imgur_album(post).await,
            MediaType::ImgurUnknown => self.download_imgur_unknown(post).await,
            MediaType::StreamableVideo => self.download_streamable_video(post).await,
            _ => {
                debug!("Unsupported URL: {:?}", post.get_url());
                *self.unsupported.lock().unwrap() += 1;
                Ok(())
            }
        };

        if let Err(e) = result {
            self.fail(e);
        }
    }

    async fn download_gallery(&mut self, post: &Post) -> Result<()> {
        let gallery = post.data.gallery_data.as_ref().unwrap();
        let media_metadata = post.data.media_metadata.as_ref().unwrap();

        // collect all the URLs for the images in the album
        for (index, item) in gallery.items.iter().enumerate() {
            let mut ext = JPG_EXTENSION;
            if let Some(media) = media_metadata.get(&item.media_id) {
                ext = media.m.split('/').last().unwrap();
            }
            let url = format!("https://{}/{}.{}", REDDIT_IMAGE_SUBDOMAIN, item.media_id, ext);
            let task = DownloadTask::from_post(post, url, ext.to_owned(), Some(index));
            self.schedule_task(task, &post).await;
        }
        Ok(())
    }

    async fn download_reddit_image(&mut self, post: &Post) -> Result<()> {
        let url = post.get_url().unwrap();
        let extension = url.split('.').last().unwrap();
        let task = DownloadTask::from_post(post, url.to_owned(), extension.to_owned(), None);
        self.schedule_task(task, &post).await;
        Ok(())
    }

    async fn download_gfycat(&mut self, post: &Post) -> Result<()> {
        let url = post.data.url.as_ref().unwrap();
        let extension = url.split('.').last().unwrap();
        match extension {
            MP4_EXTENSION => {
                let task =
                    DownloadTask::from_post(post, url.to_owned(), extension.to_owned(), None);
                self.schedule_task(task, &post).await;
            }
            _ => {
                // Convert Gfycat/Redgifs GIFs into mp4 URLs for download
                let api_prefix = if url.contains(GFYCAT_DOMAIN) {
                    GFYCAT_API_PREFIX
                } else {
                    REDGIFS_API_PREFIX
                };
                let media_id = url
                    .split('/')
                    .last()
                    .with_context(|| format!("Unsupported Gfycat URL: {}", url))?;

                let api_url = format!("{}/{}", api_prefix, media_id);
                debug!("GFY API URL: {}", api_url);

                // talk to gfycat API and get GIF information
                let response = self
                    .session
                    .get(&api_url)
                    .send()
                    .await
                    .context("Error getting response from GFYCAT API")?;
                // if the gif is not available anymore, Gfycat might send
                // a 404 response. Proceed to get the mp4 URL only if the
                // response was HTTP 200
                if response.status() == StatusCode::OK {
                    let data = response
                        .json::<GfyData>()
                        .await
                        .context("Error parsing JSON response from GFYCAT API")?;
                    let task = DownloadTask::from_post(
                        post,
                        data.gfy_item.mp4_url,
                        MP4_EXTENSION.to_owned(),
                        None,
                    );
                    self.schedule_task(task, &post).await;
                } else {
                    bail!("Gfycat API returned status code: {}", response.status());
                }
            }
        }
        Ok(())
    }

    async fn download_reddit_video(&mut self, post: &Post) -> Result<()> {
        let post_url = post.data.url.as_ref().unwrap();
        let extension = post_url.split('.').last().unwrap();

        let url = match extension {
            MP4_EXTENSION => {
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

        let mut audio_task: Option<DownloadTask> = None;

        let dash_video =
            url.split('/').last().context(format!("Unsupported reddit video URL: {}", url))?;

        if !dash_video.contains("DASH") {
            bail!("Cannot extract video url from reddit video");
        }
        // todo: find exhaustive collection of these, or figure out if they are (x, x*2) pairs
        let dash_video_only = vec!["DASH_1_2_M", "DASH_2_4_M", "DASH_4_8_M"];
        let ext = url.split('.').last().unwrap().to_owned();
        let video_task = DownloadTask::from_post(post, url.clone(), ext, None);
        if !dash_video_only.contains(&dash_video) & self.ffmpeg_available {
            let all = &url.split('/').collect::<Vec<&str>>();
            let mut result = all.split_last().unwrap().1.to_vec();
            result.push("DASH_audio.mp4");
            // dynamically generate audio URLs for reddit videos by changing the video URL
            let maybe_audio_url = result.join("/");
            // Check the mime type to see the generated URL contains an audio file
            // This can be done by checking the content type header for the given URL
            // Reddit API response does not seem to expose any easy way to figure this out
            if let Ok(exists) = check_url_has_mime_type(&maybe_audio_url, mime::MP4).await {
                if exists {
                    audio_task = Some(DownloadTask::from_post(
                        post,
                        maybe_audio_url,
                        MP4_EXTENSION.to_owned(),
                        None,
                    ));
                }
            }
        }

        if let Some(a_task) = audio_task {
            if let (Some(video_filename), Some(audio_filename)) =
                (self.schedule_task(video_task, &post).await, self.schedule_task(a_task, &post).await)
            {
                // merge the audio and video files
                if self.stitch_audio_video(&video_filename, &audio_filename).await.is_err() {
                    debug!("Error merging audio and video files");
                }
            }
        } else {
            self.schedule_task(video_task, &post).await;
        }

        Ok(())
    }

    async fn download_giphy(&mut self, post: &Post) -> Result<()> {
        let url = post.data.url.as_ref().unwrap();
        let parsed = Url::parse(url).unwrap();
        let extension = url.split('.').last().unwrap();

        if url.contains(GIPHY_MEDIA_SUBDOMAIN)
            || url.contains(GIPHY_MEDIA_SUBDOMAIN_0)
            || url.contains(GIPHY_MEDIA_SUBDOMAIN_1)
            || url.contains(GIPHY_MEDIA_SUBDOMAIN_2)
            || url.contains(GIPHY_MEDIA_SUBDOMAIN_3)
            || url.contains(GIPHY_MEDIA_SUBDOMAIN_4)
        {
            // if we encounter gif, mp4 or gifv - download as is
            match extension {
                GIF_EXTENSION | MP4_EXTENSION | GIFV_EXTENSION => {
                    let task =
                        DownloadTask::from_post(post, url.to_owned(), extension.to_owned(), None);
                    self.schedule_task(task, &post).await;
                }
                _ => {
                    // if the link points to the giphy post rather than the media link,
                    // use the scheme below to get the actual URL for the gif.
                    let path = &parsed[Position::AfterHost..Position::AfterPath];
                    let media_id = path.split('-').last().unwrap();
                    let giphy_url =
                        format!("https://{}/media/{}.gif", GIPHY_MEDIA_SUBDOMAIN, media_id);
                    let task =
                        DownloadTask::from_post(post, giphy_url, GIF_EXTENSION.to_owned(), None);
                    self.schedule_task(task, &post).await;
                }
            }
        }
        Ok(())
    }

    async fn download_imgur_gif(&mut self, post: &Post) -> Result<()> {
        let url = post.data.url.as_ref().unwrap();

        // if the extension is gifv, then replace gifv->mp4 to get the video URL
        let task = DownloadTask::from_post(
            post,
            url.replace(".gifv", ".mp4"),
            MP4_EXTENSION.to_owned(),
            None,
        );
        self.schedule_task(task, &post).await;
        Ok(())
    }

    async fn download_imgur_image(&mut self, post: &Post) -> Result<()> {
        let url = post.data.url.as_ref().unwrap();
        let extension = url.split('.').last().unwrap();

        let task = DownloadTask::from_post(post, url.to_owned(), extension.to_owned(), None);
        self.schedule_task(task, &post).await;
        Ok(())
    }

    async fn download_imgur_unknown(&mut self, post: &Post) -> Result<()> {
        let url = post.data.url.as_ref().unwrap();

        // try adding the .jpg extension to the URL
        let url = format!("{}.jpg", url);
        let success = check_url_has_mime_type(&url, mime::JPEG).await.unwrap_or(false);
        if success {
            let task = DownloadTask::from_post(post, url, JPG_EXTENSION.to_owned(), None);
            self.schedule_task(task, &post).await;
            return Ok(());
        }

        let url = format!("{}.png", url);
        let success = check_url_has_mime_type(&url, mime::PNG).await.unwrap_or(false);
        if success {
            let task = DownloadTask::from_post(post, url, PNG_EXTENSION.to_owned(), None);
            self.schedule_task(task, &post).await;
            return Ok(());
        }

        bail!("Cannot determine imgur image type");
    }

    async fn download_imgur_album(&mut self, post: &Post) -> Result<()> {
        let url = post.data.url.as_ref().unwrap();
        let mut tokens = url.split('/').collect::<Vec<&str>>();
        tokens.push("zip");
        let url = tokens.join("/");

        let task = DownloadTask::from_post(post, url, ZIP_EXTENSION.to_owned(), None);
        self.schedule_task(task, &post).await;
        Ok(())
    }

    async fn download_streamable_video(&mut self, post: &Post) -> Result<()> {
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

        if !parsed.files.contains_key(MP4_EXTENSION) {
            bail!("No mp4 file found in streamable API response")
        }

        let video_url = parsed.files.get(MP4_EXTENSION).unwrap().url.borrow().to_owned().unwrap();
        let ext = MP4_EXTENSION.to_owned();

        let task = DownloadTask::from_post(post, video_url, ext, None);
        self.schedule_task(task, &post).await;

        Ok(())
    }

    fn fail(&self, e: anyhow::Error) {
        error!("{}", e);
        *self.failed.lock().unwrap() += 1;
    }

    fn skip(&self, msg: &str) {
        debug!("{}", msg);
        *self.skipped.lock().unwrap() += 1;
    }

    async fn schedule_task(&mut self, task: DownloadTask, post: &Post) -> Option<String> {
        debug!("Received task: {:?}", task);
        {
            *self.supported.lock().unwrap() += 1;
        }

        if !self.should_download {
            let msg = format!("Found media at: {}", task.url);
            self.skip(&msg);
            return None;
        }
        let file_name = self.get_filename(&task);

        if check_path_present(&file_name)
            || check_path_present(&file_name.replace(".gif", ".mp4"))
            || check_path_present(&file_name.replace(".zip", ".jpg"))
        {
            let msg = format!("Media from url {} already downloaded. Skipping...", task.url);
            self.skip(&msg);
            return None;
        }

        let result = self.download_media(&file_name, &task.url, &post).await;
        match result {
            Ok(true) => {
                {
                    *self.downloaded.lock().unwrap() += 1;
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
                self.fail(anyhow!("Failed to download media from url: {}", task.url));
                None
            }
            Err(e) => {
                self.fail(anyhow!("Error while downloading media from url {}: {}", task.url, e));
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

        if task.extension == GIF_EXTENSION {
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
        if task.extension == ZIP_EXTENSION {
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
        }

        Err(GertError::FfmpegError("Failed to merge audio and video".into()))
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
    fn from_post(
        post: &Post,
        url: String,
        extension: String,
        index: Option<usize>,
    ) -> DownloadTask {
        DownloadTask {
            url,
            subreddit: post.data.subreddit.to_owned(),
            extension,
            post_name: post.data.name.to_owned(),
            post_title: post.data.title.clone().unwrap(),
            index,
        }
    }
}
