/// Prepare Video
///
/// Goal:  Convert MP4 into MP4 (av1 + opus)
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

fn args_av1<'a>(command: &mut Command, quality: u8) {
    command
        .arg("-c:v").arg("libaom-av1")
        .arg("-crf").arg(&*format!("{}", quality))
        .arg("-b:v").arg("0")
        .arg("-c:a").arg("libopus")
        .arg("-strict").arg("experimental");
}

fn convert(input: &str, output: &str, loudnorm: &Loudnorm, size: &str, quality: u8) {
    let mut command = Command::new(CPULIMIT_PATH);
    command.arg("-l").arg(CPULIMIT)
        .arg(FFMPEG_PATH)
        .arg("-y")
        .arg("-i").arg(input)
        .arg("-af")
        .arg(&*loudnorm.convert_af());

    args_av1(&mut command, quality);
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

    let (size, quality) = if &*dest == "youtube" {
        ("1280:720", 30)
    } else if &*dest == "mikedilger" {
        ("1024:576", 45)
    } else {
        ("1280:720", 30)
    };

    println!("Analyzing loudnorm...");
    let loudnorm = Loudnorm::from_analyze(&input_file);

    println!("Converting video (second pass w/ multiple functions)...");
    convert(&input_file, "stage1.mp4", &loudnorm, size, quality);

    println!("Stripping metadata...");
    strip_metadata("stage1.mp4", "stage2.mp4", &title);
}
