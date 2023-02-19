# Gert

A command line tool to download media from Reddit

- Supports:
  - Reddit: PNG/JPG images, GIFs, Image galleries, videos
  - Giphy: GIFs
  - Imgur: Direct images, GIFVs and albums
  - Gfycat/Redgifs: GIFs
  - Streamable: videos
- GIF/GIFV from Imgur/Gfycat/Redgifs are downloaded as mp4

## Installation

### Prerequisites

There is a soft dependency on ffmpeg, for installation instructions follow this [link](https://www.ffmpeg.org/download.html).

You can skip it but without it:

- Videos hosted on reddit itself (v.redd.it) won't have sound
- Gifs won't be automatically converted to .mp4

#### Using cargo

If you already have Rust installed, you can install using `cargo`:

```shell script
cargo install gert
```

#### Using homebrew

```shell script
brew tap mcdallas/gert
brew install gert
```

#### Github Release

just grab the [latest release](https://github.com/mcdallas/gert/releases/latest) for your OS

## Running

Simply pass the names of the subreddits you want to download media from with (multiple) `-s` flags

```bash
gert -s wallpapers -s earthporn
```

![gert4](https://user-images.githubusercontent.com/15388116/200098386-762a7655-9bb0-43e8-a645-09fdb65c886d.gif)

To download media from a single post/collection just pass the url of the post

```bash
gert https://old.reddit.com/r/wallpapers/comments/tckky1/some_walls_from_my_collections_vol6/
```

## Command line options

```bash
Simple CLI tool to download media from Reddit

USAGE:
    gert [FLAGS] [OPTIONS] --subreddit <SUBREDDIT>...

FLAGS:
    -c  --conserve-gifs     Don't convert gifs to MP4
        --debug             Show the current config being used
    -r, --dry-run           Dry run and print the URLs of saved media to download
    -h, --help              Prints help information
    -H, --human-readable    Use human readable names for files
    -V, --version           Prints version information

OPTIONS:
    -e, --from-env <ENV_FILE>         Set a custom .env style file with secrets
    -f, --feed <feed>                 Feed to download from [default: hot]  [possible values: hot, new, top, rising]
    -l, --limit <LIMIT>               Limit the number of posts to download [default: 25]
    -m, --match <MATCH>               Pass a regex expresion to filter the title of the post
    -o, --output <DATA_DIR>           Directory to save the media to [default: .]
    -p, --period <PERIOD>             Time period to download from [default: day]  [possible values: now, hour, day, week, month, year, all]
    -s, --subreddit <SUBREDDIT>...    Download media from these subreddit
    -u, --upvotes <NUM>               Minimum number of upvotes to download [default: 0]
```

### Optional Authentication with Reddit

Authentication is not required but if you want a more generous rate limit you can create a new app in reddit and pass your credentials to gert

1. Create a new script application at https://www.reddit.com/prefs/apps
   - Click on create an app at the bottom of the page
   - Input a name for your application, for example: `gert`
   - Choose "script" as the type of application
   - Set "http://localhost:8080" or any other URL for the redirect url
   - Click on "create app" - you should now see the application has been created
   - Under your application name, you should see a random string - that is your client ID
   - The random string next to the field "secret" is your client secret
2. Copy the client ID and client secret information returned
3. Create a .env file with the following keys, for example `gert.env`:

```shell script
CLIENT_ID="<client_id>"
CLIENT_SECRET="<client_secret>"
USERNAME="<username>"
PASSWORD="<password>"
```

_NOTE_: If you have 2FA enabled, please make sure you set `PASSWORD=<password>:<2FA_TOTP_token>` instead

### Credits

based on https://github.com/manojkarthick/reddsaver
