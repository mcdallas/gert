use std::env;

use clap::{crate_version, App, Arg};
use env_logger::Env;
use log::{debug, info, warn};

use auth::Client;

use crate::download::Downloader;
use crate::errors::GertError;
use crate::errors::GertError::DataDirNotFound;
use crate::user::User;
use crate::utils::*;
use crate::subreddit::Subreddit;
use crate::structures::Post;

mod auth;
mod download;
mod errors;
mod structures;
mod user;
mod utils;
mod subreddit;

#[tokio::main]
async fn main() -> Result<(), GertError> {
    
    let matches = App::new("Gert")
        .version(crate_version!())
        .author("Manoj Karthick Selva Kumar")
        .about("Simple CLI tool to download saved media from Reddit")
        .arg(
            Arg::with_name("environment")
                .short("e")
                .long("from-env")
                .value_name("ENV_FILE")
                .help("Set a custom .env style file with secrets")
                .default_value(".env")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("match")
                .short("m")
                .long("match")
                .value_name("MATCH")
                .help("Pass a regex expresion to filter the title of the post")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("output_directory")
                .short("o")
                .long("output")
                .value_name("DATA_DIR")
                .help("Directory to save the media to")
                .default_value(".")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("debug")
                .long("debug")
                .takes_value(false)
                .help("Show the current config being used"),
        )
        .arg(
            Arg::with_name("dry_run")
                .short("r")
                .long("dry-run")
                .takes_value(false)
                .help("Dry run and print the URLs of saved media to download"),
        )
        .arg(
            Arg::with_name("human_readable")
                .short("H")
                .long("human-readable")
                .takes_value(false)
                .help("Use human readable names for files"),
        )
        .arg(
            Arg::with_name("limit")
                .short("l")
                .long("limit")
                .value_name("LIMIT")
                .help("Limit the number of posts to download")
                .takes_value(true)
                .default_value("25"),
        )
        .arg(
            Arg::with_name("subreddits")
                .short("s")
                .long("subreddit")
                .multiple(true)
                .value_name("SUBREDDIT")
                .value_delimiter(",")
                .help("Download media from these subreddit")
                .takes_value(true)
                .required(true)
        )
        .arg(
            Arg::with_name("period")
                .short("p")
                .long("period")
                .value_name("PERIOD")
                .help("Time period to download from")
                .takes_value(true)
                .possible_values(&["now", "hour", "day", "week", "month", "year", "all"])
                .default_value("day")
        )
        .arg(
            Arg::with_name("feed")
                .short("f")
                .long("feed")
                .value_name("feed")
                .help("Feed to download from")
                .takes_value(true)
                .possible_values(&["hot", "new", "top", "rising"])
                .default_value("hot")
        )
        .get_matches();

    let env_file = matches.value_of("environment").unwrap();
    let data_directory = String::from(matches.value_of("output_directory").unwrap());
    // generate the URLs to download from without actually downloading the media
    let should_download = !matches.is_present("dry_run");
    // check if ffmpeg is present for combining video streams
    let ffmpeg_available = application_present(String::from("ffmpeg"));
    // generate human readable file names instead of MD5 Hashed file names
    let use_human_readable = matches.is_present("human_readable");
    // restrict downloads to these subreddits
    let subreddits: Vec<&str> = matches.values_of("subreddits").unwrap().collect();
    let limit = matches.value_of("limit").unwrap().parse::<u32>().expect("Limit must be a number");
    let period = matches.value_of("period");
    let feed = matches.value_of("feed").unwrap();
    let pattern = match matches.value_of("match") {
        Some(pattern) => regex::Regex::new(pattern).expect("Invalid regex pattern"),
        None => regex::Regex::new(".*").unwrap(),
    };
    
    // initialize environment from the .env file
    dotenv::from_filename(env_file).ok();

    // initialize logger for the app and set logging level to info if no environment variable present
    let env = Env::default().filter("RS_LOG").default_filter_or("info");
    env_logger::Builder::from_env(env).init();

    let client_id = env::var("CLIENT_ID")?;
    let client_secret = env::var("CLIENT_SECRET")?;
    let username = env::var("USERNAME")?;
    let password = env::var("PASSWORD")?;
    let user_agent = get_user_agent_string(&username);

    if !check_path_present(&data_directory) {
        return Err(DataDirNotFound);
    }

    // if the option is show-config, show the configuration and return immediately
    if matches.is_present("debug") {
        info!("Current configuration:");
        info!("ENVIRONMENT_FILE = {}", &env_file);
        info!("DATA_DIRECTORY = {}", &data_directory);
        info!("CLIENT_ID = {}", &client_id);
        info!("CLIENT_SECRET = {}", mask_sensitive(&client_secret));
        info!("USERNAME = {}", &username);
        info!("PASSWORD = {}", mask_sensitive(&password));
        info!("USER_AGENT = {}", &user_agent);
        info!("SUBREDDITS = {}", &subreddits.join(","));
        info!("FFMPEG AVAILABLE = {}", ffmpeg_available);
        info!("LIMIT = {}", limit);
        info!("PERIOD = {}", period.unwrap());
        info!("FEED = {}", feed);
        info!("MATCH = {}", pattern.as_str());

        return Ok(());
    }

    if !ffmpeg_available {
        warn!(
            "No ffmpeg Installation available. \
            Videos hosted by Reddit use separate video and audio streams. \
            Ffmpeg needs be installed to combine the audio and video into a single mp4."
        );
    }
    ;
    let session = reqwest::Client::builder()
        .cookie_store(true)
        .user_agent(get_user_agent_string(&username))
        .build()?;


    let client = Client::new(&client_id, &client_secret, &username, &password, &session);
    // login to reddit using the credentials provided and get API bearer token
    let auth =
        client.login().await?;
  
    info!("Successfully logged in to Reddit as {}", username);
    debug!("Authentication details: {:#?}", auth);

    // get information about the user to display
    let user = User::new(&auth, &username, &session);

    let user_info = user.about().await?;
    info!("The user details are: ");
    info!("Account name: {:#?}", user_info.data.name);
    info!("Account ID: {:#?}", user_info.data.id);
    info!("Comment Karma: {:#?}", user_info.data.comment_karma);
    info!("Link Karma: {:#?}", user_info.data.link_karma);

    info!("Starting data gathering from Reddit. This might take some time. Hold on....");

    let mut posts: Vec<Post> = Vec::with_capacity(limit as usize * subreddits.len());
    for subreddit in &subreddits {
        let listing = Subreddit::new(subreddit).get_feed(feed, limit, period).await?;
        posts.extend(
            listing.data.children
            .into_iter()
            .filter(|post| post.data.url.is_some())
            .filter(|post| pattern.is_match(post.data.title.as_ref().unwrap_or(&"".to_string())))
        );
    }

    let downloader = Downloader::new(
        posts,
        &data_directory,
        should_download,
        use_human_readable,
        ffmpeg_available,
        &session,
    );

    downloader.run().await?;

    Ok(())
}
