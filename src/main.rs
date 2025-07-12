use std::env;

use clap::{crate_version, App, Arg};
use env_logger::Env;
use log::{debug, info, warn};

use auth::Client;

use crate::download::Downloader;
use crate::errors::GertError;
use crate::errors::GertError::DataDirNotFound;
use crate::structs::{Post, SingleListing};
use crate::subreddit::Subreddit;
use crate::user::User;
use crate::utils::*;

mod auth;
mod download;
mod errors;
mod structs;
mod subreddit;
mod user;
mod utils;

fn exit(msg: &str) -> ! {
    let err = clap::Error::with_description(msg, clap::ErrorKind::InvalidValue);
    err.exit();
}

#[tokio::main]
async fn main() -> Result<(), GertError> {
    let matches = App::new("Gert")
        .version(crate_version!())
        .author("Mike Dallas")
        .about("Simple CLI tool to download media from Reddit")
        .arg(
            Arg::with_name("url")
                .value_name("URL")
                .help("URL of a single post to download")
                .takes_value(true)
                .required_unless("subreddit")
                .conflicts_with_all(&["subreddit", "period", "feed", "limit", "match", "upvotes"]),
        )
        .arg(
            Arg::with_name("environment")
                .short("e")
                .long("from-env")
                .value_name("ENV_FILE")
                .help("Set a custom .env style file with secrets")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("match")
                .short("m")
                .long("match")
                .value_name("MATCH")
                .help("Pass a regular expresion to filter the title of the post")
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
                .help("Download media from these subreddits")
                .takes_value(true)
                .required_unless("url")
                .conflicts_with("url"),
        )
        .arg(
            Arg::with_name("period")
                .short("p")
                .long("period")
                .value_name("PERIOD")
                .help("Time period to download from")
                .takes_value(true)
                .possible_values(&["now", "hour", "day", "week", "month", "year", "all"])
                .default_value("day"),
        )
        .arg(
            Arg::with_name("feed")
                .short("f")
                .long("feed")
                .value_name("feed")
                .help("Feed to download from")
                .takes_value(true)
                .possible_values(&["hot", "new", "top", "rising"])
                .default_value("hot"),
        )
        .arg(
            Arg::with_name("upvotes")
                .short("u")
                .long("upvotes")
                .value_name("NUM")
                .help("Minimum number of upvotes to download")
                .takes_value(true)
                .default_value("0"),
        )
        .arg(
            Arg::with_name("conserve_gifs")
                .short("c")
                .long("conserve-gifs")
                .value_name("conserve_gifs")
                .help("Disable gif to mp4 conversion")
                .takes_value(false),
        )
        .get_matches();

    let env_file = matches.value_of("environment");
    let data_directory = String::from(matches.value_of("output_directory").unwrap());
    // generate the URLs to download from without actually downloading the media
    let should_download = !matches.is_present("dry_run");
    // check if ffmpeg is present for combining video streams
    let ffmpeg_available = application_present(String::from("ffmpeg"));
    // generate human readable file names instead of MD5 Hashed file names
    let use_human_readable = matches.is_present("human_readable");
    // restrict downloads to these subreddits
    let upvotes = matches
        .value_of("upvotes")
        .unwrap()
        .parse::<i64>()
        .unwrap_or_else(|_| exit("Upvotes must be a number"));

    let subreddits: Vec<&str> = match matches.is_present("subreddits") {
        true => matches.values_of("subreddits").unwrap().collect(),
        false => Vec::new(),
    };

    let single_url = match matches.value_of("url") {
        Some(url) => {
            let parsed = url.parse::<url::Url>();
            if parsed.is_err() {
                exit("Invalid URL");
            }
            Some(parsed.unwrap())
        }
        None => None,
    };

    let limit = match matches.value_of("limit").unwrap().parse::<u32>() {
        Ok(limit) => limit,
        Err(_) => exit("Limit must be a number"),
    };
    let period = matches.value_of("period");
    let feed = matches.value_of("feed").unwrap();
    let pattern = match matches.value_of("match") {
        Some(pattern) => match regex::Regex::new(pattern) {
            Ok(reg) => reg,
            Err(_) => exit("Invalid regex pattern"),
        },
        None => regex::Regex::new(".*").unwrap(),
    };
    let conserve_gifs: bool = matches.is_present("conserve_gifs");

    // initialize logger for the app and set logging level to info if no environment variable present
    let env = Env::default().filter("RUST_LOG").default_filter_or("info");
    env_logger::Builder::from_env(env).init();

    // if the option is --debug, show the configuration and return immediately
    if matches.is_present("debug") {
        info!("Current configuration:");
        info!("ENVIRONMENT_FILE = {}", &env_file.unwrap_or("None"));
        info!("DATA_DIRECTORY = {}", &data_directory);
        if let Some(envfile) = env_file {
            let maybe_userenv = parse_env_file(envfile);
            match maybe_userenv {
                Ok(userenv) => {
                    info!("CLIENT_ID = {}", &userenv.client_id);
                    info!("CLIENT_SECRET = {}", mask_sensitive(&userenv.client_secret));
                    info!("USERNAME = {}", &userenv.username);
                    info!("PASSWORD = {}", mask_sensitive(&userenv.password));
                    info!("USER_AGENT = {}", get_user_agent_string(&userenv.username));
                }
                Err(e) => {
                    warn!("Error parsing environment file: {}", e);
                }
            }
        } else {
            info!("USER_AGENT = {}", get_user_agent_string("anon"));
        }
        info!("SUBREDDITS = {}", &subreddits.join(","));
        info!("FFMPEG AVAILABLE = {}", ffmpeg_available);
        info!("LIMIT = {}", limit);
        info!("PERIOD = {}", period.unwrap());
        info!("FEED = {}", feed);
        info!("MATCH = {}", pattern.as_str());
        info!("CONSERVE GIFS = {}", conserve_gifs);

        return Ok(());
    }

    let session = match env_file {
        Some(envfile) => {
            let user_env = parse_env_file(envfile)?;

            let client_sess = reqwest::Client::builder()
                .cookie_store(true)
                .user_agent(get_user_agent_string(&user_env.username))
                .build()?;

            let client = Client::new(
                &user_env.client_id,
                &user_env.client_secret,
                &user_env.username,
                &user_env.password,
                &client_sess,
            );
            // login to reddit using the credentials provided and get API bearer token
            let auth = client.login().await?;

            info!("Successfully logged in to Reddit as {}", user_env.username);
            debug!("Authentication details: {:#?}", auth);

            // get information about the user to display
            let user = User::new(&auth, &user_env.username, &client_sess);

            let user_info = user.about().await?;

            info!("The user details are: ");
            info!("Account name: {:#?}", user_info.data.name);
            info!("Account ID: {:#?}", user_info.data.id);
            info!("Comment Karma: {:#?}", user_info.data.comment_karma);
            info!("Link Karma: {:#?}", user_info.data.link_karma);

            client_sess
        }
        None => {
            info!("No environment file provided, using default values");
            reqwest::Client::builder()
                .cookie_store(true)
                .user_agent(get_user_agent_string("anon"))
                .build()?
        }
    };

    if !check_path_present(&data_directory) {
        return Err(DataDirNotFound);
    }

    if !ffmpeg_available {
        warn!(
            "No ffmpeg Installation available. \
            Videos hosted by Reddit use separate video and audio streams. \
            Ffmpeg needs be installed to combine the audio and video into a single mp4."
        );
    };

    info!("Starting data gathering from Reddit. This might take some time. Hold on....");

    let mut posts: Vec<Post> = Vec::with_capacity(limit as usize * subreddits.len());
    if let Some(url) = single_url {

        let mut url = url.as_str();

        let temp_client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .user_agent(get_user_agent_string("anon"))
                .build()?;
        // Check for redirections with a head request
        let response = temp_client
            .head(url)
            .send()
            .await
            .map_err(|_| GertError::UrlNotFound(url.to_string()))?;

        if response.status() == reqwest::StatusCode::MOVED_PERMANENTLY {
            url = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|h| h.to_str().ok())
                .unwrap_or_else(|| exit("No redirection location found"));
        }
        // Strip url params
        let url = if url.contains('?') {
            &url[..url.find('?').unwrap()]
        } else {
            url
        };

        let url = format!("{}.json", url);
        let single_listing: SingleListing = match session.get(&url).send().await {
            Ok(response) => response.json().await.map_err(|_| GertError::JsonParseError(url))?,
            Err(_) => exit(&format!("Error fetching data from {}", &url)),
        };

        let post = single_listing.0.data.children.into_iter().next().unwrap();
        if post.data.url.is_none() {
            exit("Post contains no media")
        }
        posts.push(post);
    } else {
        for subreddit in &subreddits {
            let subposts =
                Subreddit::new(subreddit, &session).get_posts(feed, limit, period).await?;
            posts.extend(
                subposts
                    .into_iter()
                    .filter(|post| {
                        post.data.url.is_some() && !post.data.is_self && post.data.score > upvotes
                    })
                    .filter(|post| {
                        pattern.is_match(post.data.title.as_ref().unwrap_or(&"".to_string()))
                    }),
            );
        }
    }
    let mut downloader = Downloader::new(
        posts,
        &data_directory,
        should_download,
        use_human_readable,
        ffmpeg_available,
        session,
        conserve_gifs,
    );

    downloader.run().await?;

    Ok(())
}
