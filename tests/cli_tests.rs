
use std::fs;
use std::process::Command;
use std::path::Path;
use assert_cmd::prelude::*;

const PATH: &str = "test-data";

struct TestCase {
    url: &'static str,
    files: Vec<File>,

}

struct File {
    filename: &'static str,
    subreddit: &'static str,
    extension: &'static str,
    filesize: u64,
}

impl File {
    fn filepath(&self) -> String {
        format!("{}/{}/{}.{}", PATH, self.subreddit, self.filename, self.extension)
    }
}

#[tokio::test]
async fn test_gif_to_mp4() {
    // Gif to mp4 conversion
    let file = File {
        filename: "d54597fb25de9af539b97e79ef6c7c00",
        subreddit: "gifs",
        extension: "mp4",
        filesize: 326981,
    };
    let test_case = TestCase {
        
        url: "https://old.reddit.com/r/gifs/comments/ynpamf/i_drew_this_pixel_art_scene_using_6_colors_and/",
        files: vec![file],
    };

    run_test_case(test_case).await;
}

#[tokio::test]
async fn test_audio_video_stitch() {
    // Test merging audio and video files

    let file = File {
        
        filename: "88d27c566910c4667076fd40b3e8b00e",
        subreddit: "therewasanattempt",
        extension: "mp4",
        filesize: 2234639,
    };
    let test_case = TestCase {
       
        url: "https://www.reddit.com/r/therewasanattempt/comments/ynowo3/to_be_funny_in_a_drive_thru/",
        files: vec![file],
    };

    run_test_case(test_case).await;
}

#[tokio::test]
async fn test_giphy() {

    let file = File {
        filename: "c3bcdb14dc4f7627e3268d15dc7a3dee",
        subreddit: "gifs",
        extension: "mp4",
        filesize: 110300,
    };
    let test_case = TestCase {
        // Gif to webm conversion
        url: "https://www.reddit.com/r/gifs/comments/idqrmp/giphy_lightening_storm_in_sc/",
        files: vec![file],
    };

    run_test_case(test_case).await;
}

#[tokio::test]
async fn test_imgur_unknown() {
    let file = File {
        filename: "fef2711051128c9e1ed5301a7e2055ac",
        subreddit: "wallpaper",
        extension: "jpg",
        filesize: 3915418,
    };
    let test_case = TestCase {
        url: "https://www.reddit.com/r/wallpaper/comments/ym5gyp/7680x4320_durban_krishna_temple_in_south_africa/",
        files: vec![file],
    };

    run_test_case(test_case).await;
}

#[tokio::test]
async fn test_streamable() {
    let file = File {
        filename: "25e18a8e5db33f7059a1bc867dcc00b0",
        subreddit: "nba",
        extension: "mp4",
        filesize: 10004941,
    };
    let test_case = TestCase {
        url: "https://www.reddit.com/r/nba/comments/17sk844/highlight_wemby_with_the_deep_3_over_rudy_gobert/",
        files: vec![file],
    };

    run_test_case(test_case).await;
}

#[tokio::test]
async fn test_redgif() {
    let file = File {
        filename: "3b436cfd5a67f03610036df4c9b018a9",
        subreddit: "WatchItForThePlot",
        extension: "mp4",
        filesize: 8315394,
    };
    let test_case = TestCase {
        url: "https://old.reddit.com/r/WatchItForThePlot/comments/ynrm94/kristen_davis_sex_and_the_city/",
        files: vec![file],
    };

    run_test_case(test_case).await;
}

#[tokio::test]
async fn test_imgur_collection() {
    let files = vec![
        File {
            filename: "d357540fde037f4d0a3583ca087ab945_1",
            subreddit: "IBM",
            extension: "jpg",
            filesize: 866201,
        },
        File {
            filename: "d357540fde037f4d0a3583ca087ab945",
            subreddit: "IBM",
            extension: "jpg",
            filesize: 866086,
        },
    ];

    let test_case = TestCase {
        url: "https://www.reddit.com/r/IBM/comments/yh78ff/gorgeous_wallpapers_featuring_ibm_pcjr_created_by/",
        files,
    };
    run_test_case(test_case).await;
}

#[tokio::test]
async fn test_gallery() {
    let files = vec![
        File {
            filename: "fb7b24b0b20a2e4f0e042bea5417794f",
            subreddit: "iWallpaper",
            extension: "png",
            filesize: 1901433,
        },
        File {
            filename: "ebde5e357b41be89d5c969c125b47b26_2",
            subreddit: "iWallpaper",
            extension: "png",
            filesize: 10933147,
        },
        File {
            filename: "e10e617c7633df37c058e365688e51ed_3",
            subreddit: "iWallpaper",
            extension: "png",
            filesize: 8788703,
        },
        File {
            filename: "9a8bf55fb708b9dfc51c50a076acfde2_4",
            subreddit: "iWallpaper",
            extension: "png",
            filesize: 8496816,
        },
        File {
            filename: "c2a136ecd1ce1d03d6653f00636410ea_5",
            subreddit: "iWallpaper",
            extension: "png",
            filesize: 9153176,
        },
        File {
            filename: "b2ce059186d0f68b34709aef89137980_6",
            subreddit: "iWallpaper",
            extension: "png",
            filesize: 8086699,
        },
    ];
    let test_case = TestCase {
        url: "https://old.reddit.com/r/iWallpaper/comments/xb0vw6/cyberpunk_girl_looking_out_the_window_pack/",
        files,
    };
    run_test_case(test_case).await;
}

async fn run_test_case(test_case: TestCase) {
    // Get the path of the compiled binary
    let mut cmd = Command::cargo_bin("gert").unwrap();


    let path = Path::new(PATH);
    if !path.exists() {
        fs::create_dir(path).unwrap();
    }

    let output = cmd
        .arg(test_case.url)
        .arg("-o")
        .arg(PATH)
        .output()
        .expect("Failed to execute command");

    println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(output.status.success(), "Command did not run successfully");

    for file in test_case.files.iter() {
        let expected_file_path = file.filepath();
        let file_exists = Path::new(&expected_file_path).exists();
        assert!(file_exists, "The file was not downloaded");
        let file_size = fs::metadata(&expected_file_path).unwrap().len();
        // Clean up
        if file_exists {
            fs::remove_file(&expected_file_path).unwrap();
        }
        assert_eq!(file_size, file.filesize, "The file size is incorrect");
    }

}
