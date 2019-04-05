/// Prepare Video
///
/// Goal:  Convert MP4 into WEBM (vp9 + opus)
///        Normalize the audio (loudnorm)
///        Strip out the metadata

use std::env;
use std::process::Command;

mod loudnorm;
use loudnorm::*;

const CPULIMIT_PATH: &'static str = "/usr/bin/cpulimit";
const FFMPEG_PATH: &'static str = "/usr/bin/ffmpeg";

const CPULIMIT: &'static str = "600";

// This is close to what YouTube uses
const VP9_BITRATE: &'static str = "1.4M";
// NOTE: YouTube compresses like this (VP9 in webm):
//   0- Youtube compresses to 4320p (8K) (7680×4320) with a bit rate  21.2 Mbps
//   1- Youtube compresses to 2160p (4K) (3840×2160) with a bit rate  17.3 Mbps
//   2- Youtube compresses to (2K) 1440p (2560×1440) with a bit rate  8.589 Mbps
//   3- Youtube compresses 1080p (1920×1080) with a bit rate  2.567 Mbps
//   4- Youtube compresses to 720p (1280×720) with a bit rate  1.468 Mbps
//   5- Youtube compresses to 480p (854×480) with a bit rate  0.727 Mbps
//   6- Youtube compresses to 360p  (640×360) with a bit rate  0.373 Mbps
//   7- Youtube compresses 240p (426×240) with a bit rate  0.157 Mbps
//   8- Youtube compresses to 144p (256×144) with a bit rate  0.085 Mbps

const USAGE: &'static str = "USAGE: prepvideo <inputfile> <title>";

fn analyze_vp9(input: &str) {
    let mut command = Command::new(CPULIMIT_PATH);
    command.arg("-l").arg(CPULIMIT)
        .arg(FFMPEG_PATH)
        .arg("-i").arg(input)
        .arg("-c:v").arg("libvpx-vp9")
        .arg("-b:v").arg(VP9_BITRATE)
        .arg("-pass").arg("1");
    args_shrink(&mut command);
    let output = command
        .arg("-an") // no audio
        .arg("-f").arg("webm")
        .arg("-")
        .output()
        .expect("failed to analyze video for vp9 pass1");

    let stderr_str = String::from_utf8_lossy(&*output.stderr).to_string();
    if ! output.status.success() {
        panic!("Failed to run ffmpeg to analyze for loudnorm. Stderr follows.\n{}",
               stderr_str);
    }
}

fn args_shrink<'a>(command: &mut Command) {
    command
        .arg("-vf")
        .arg("scale=1280:720");
}

fn args_vp9<'a>(command: &mut Command) {
    command
        .arg("-c:v").arg("libvpx-vp9")
        .arg("-b:v").arg(VP9_BITRATE)
        .arg("-pass").arg("2")
        .arg("-c:a").arg("libopus")
        .arg("-f").arg("webm");
}

fn convert(input: &str, output: &str, loudnorm: &Loudnorm) {
    let mut command = Command::new(CPULIMIT_PATH);
    command.arg("-l").arg(CPULIMIT)
        .arg(FFMPEG_PATH)
        .arg("-y")
        .arg("-i").arg(input)
        .arg("-af")
        .arg(&*loudnorm.convert_af());

    args_vp9(&mut command);
    args_shrink(&mut command);

    let output = command
        .arg(output)
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
    let output = Command::new(FFMPEG_PATH)
        .arg("-y") // overwrite output files w/o asking
        .arg("-i").arg(input)
        .arg("-c").arg("copy")
        .arg("-map_metadata").arg("-1")
        .arg("-metadata").arg(format!("title={}",title))
        .arg(output)
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

    println!("Analyzing loudnorm...");
    let loudnorm = Loudnorm::from_analyze(&input_file);

    println!("Analyzing conversion (first pass)...");
    analyze_vp9(&input_file);

    println!("Converting video (second pass w/ multiple functions)...");
    convert(&input_file, "stage1.webm", &loudnorm);

    println!("Stripping metadata...");
    strip_metadata("stage1.webm", "stage2.webm", &title);
}
