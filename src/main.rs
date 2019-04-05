/// Prepare Video
///
/// Goal:  Convert MP4 into WEBM (vp9 + opus)
///        Shrink to a nice small format
///        Normalize the audio (loudnorm)
///        Strip out the metadata

use std::env;
use std::process::Command;

mod loudnorm;
use loudnorm::*;

const CPULIMIT_PATH: &'static str = "/usr/bin/cpulimit";
const FFMPEG_PATH: &'static str = "/usr/bin/ffmpeg";

const CPULIMIT: &'static str = "600";

const USAGE: &'static str = "USAGE: prepvideo <inputfile> <title> <youtube|mikedilger>";

fn args_shrink<'a>(command: &mut Command, size: &str) {
    command
        .arg("-vf")
        .arg(&*format!("scale={}", size));
}

fn args_vp9<'a>(command: &mut Command, quality: u8) {
    command
        .arg("-c:v").arg("libvpx-vp9")
        .arg("-crf").arg(&*format!("{}", quality))
        .arg("-b:v").arg("0")
        .arg("-c:a").arg("libopus");
}

fn convert(input: &str, output: &str, loudnorm: &Loudnorm, size: &str, quality: u8) {
    let mut command = Command::new(CPULIMIT_PATH);
    command.arg("-l").arg(CPULIMIT)
        .arg(FFMPEG_PATH)
        .arg("-y")
        .arg("-i").arg(input)
        .arg("-af")
        .arg(&*loudnorm.convert_af());

    args_vp9(&mut command, quality);
    args_shrink(&mut command, size);
    command.arg(output);

    println!("{:?}", command);

    let output = command
        .output()
        .expect("failed to execute ffmpeg");

    if ! output.status.success() {
        let stderr_str = String::from_utf8_lossy(&*output.stderr).to_string();
        panic!("Failed to run ffmpeg multi command.  Stderr follows.\n{}",
               stderr_str);
    }
}

fn strip_metadata(input: &str, output: &str, title: &str)
{
    let mut command = Command::new(FFMPEG_PATH);
    command
        .arg("-y") // overwrite output files w/o asking
        .arg("-i").arg(input)
        .arg("-c").arg("copy")
        .arg("-map_metadata").arg("-1")
        .arg("-metadata").arg(format!("title={}",title))
        .arg(output);

    println!("{:?}", command);

    let output = command
        .output()
        .expect("failed to execute ffmpeg");

    if ! output.status.success() {
        let stderr_str = String::from_utf8_lossy(&*output.stderr).to_string();
        panic!("Failed to run ffmpeg to strip metadata.  Stderr follows.\n{}",
               stderr_str);
    }
}

fn main() {
    let mut args = env::args();

    let _bin = args.next().expect(USAGE);
    let input_file = args.next().expect(USAGE);
    let title = args.next().expect(USAGE);
    let dest = args.next().expect(USAGE);

    // vp9 quality 0-63, recommended 15-35, with 31 recd for 1080p HD
    // See https://developers.google.com/media/vp9/settings/vod/
    let (size, quality) = if &*dest == "youtube" {
        ("1280:720", 32)
    } else if &*dest == "mikedilger" {
        ("640:360", 36)
        // ("1024:576", 35)
    } else {
        ("1280:720", 32)
    };

    println!("Analyzing loudnorm...");
    let loudnorm = Loudnorm::from_analyze(&input_file);

    println!("Converting video (second pass w/ multiple functions)...");
    convert(&input_file, "stage1.webm", &loudnorm, size, quality);

    println!("Stripping metadata...");
    strip_metadata("stage1.webm", "stage2.webm", &title);
}
