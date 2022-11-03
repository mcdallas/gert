# Gert

* Command line tool to download media from Reddit
* Supports:
  - Reddit: PNG/JPG images, GIFs, Image galleries, videos
  - Giphy: GIFs
  - Imgur: Direct images and GIFVs
  - Gfycat/Redgifs: GIFs
* GIF/GIFV from Imgur/Gfycat/Redgifs are downloaded as mp4
* Does *not* support downloading images from Imgur post links

## Installation

### Prerequisites 

To download videos hosted by Reddit, you need to have ffmpeg installed.
Follow this [link](https://www.ffmpeg.org/download.html) for installation instructions.

#### Using cargo

If you already have Rust installed, you can also install using `cargo`: 
```shell script
cargo install gert
```

## Running

1. Create a new script application at https://www.reddit.com/prefs/apps
    * Click on create an app at the bottom of the page
    * Input a name for your application, for example: <username>-gert
    * Choose "script" as the type of application
    * Set "http://localhost:8080" or any other URL for the redirect url
    * Click on "create app" - you should now see the application has been created
    * Under your application name, you should see a random string - that is your client ID
    * The random string next to the field "secret" is your client secret 
2. Copy the client ID and client secret information returned
3. Create a .env file with the following keys, for example `gert.env`:  
```shell script
CLIENT_ID="<client_id>"
CLIENT_SECRET="<client_secret>"
USERNAME="<username>"
PASSWORD="<password>"
```
_NOTE_: If you have 2FA enabled, please make sure you set `PASSWORD=<password>:<2FA_TOTP_token>` instead
