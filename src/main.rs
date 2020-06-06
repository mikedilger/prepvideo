/// Prepare Video
///
/// Goal:  Convert MP4 into WEBM (vp9 + opus)
///        Shrink to a nice small format
///        Normalize the audio (loudnorm)
///        Strip out the metadata

use std::env;
use std::process::Command;
use std::path::{Path, PathBuf};
use std::ffi::OsStr;

mod loudnorm;
use loudnorm::*;

const CPULIMIT_PATH: &'static str = "/usr/bin/cpulimit";
const FFMPEG_PATH: &'static str = "/usr/bin/ffmpeg";

const CPULIMIT: &'static str = "600";

const USAGE: &'static str = "USAGE: prepvideo <inputfile> <title> <1920|1280|1024|854|640>";

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

fn cwd_files_with_extension(ext: &str) -> Vec<PathBuf> {
    let mut inputfiles: Vec<PathBuf> = Vec::new();
    let readdir = std::fs::read_dir(".").unwrap();
    for entry in readdir {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            if path.extension().unwrap() == ext {
                inputfiles.push(path.to_owned());
            }
        }
    }
    inputfiles.sort_unstable();
    inputfiles
}

fn cat_files<P: AsRef<Path> + AsRef<OsStr>>(inputfiles: Vec<PathBuf>, outputname: P) {
    let mut command = Command::new(FFMPEG_PATH);
    for infile in &inputfiles {
        println!("{:?}", infile);
        command
            .arg("-i")
            .arg(infile);
    }
    command.arg("-filter_complex");
    let mut nextarg: String = String::new();
    for (i,_infile) in inputfiles.iter().enumerate() {
        nextarg.push_str(&*format!("[{}:v:0][{}:a:0]", i, i));
    }
    nextarg.push_str(&*format!("concat=n={}:v=1:a=1[outv][outa]", inputfiles.len()));
    command.arg(nextarg);
    command.arg("-map")
        .arg("[outv]");
    command.arg("-map")
        .arg("[outa]");
    command.arg(outputname);

    let output = command
        .output()
        .expect("failed to execute ffmpeg");

    if ! output.status.success() {
        let stderr_str = String::from_utf8_lossy(&*output.stderr).to_string();
        panic!("Failed to run ffmpeg multi command.  Stderr follows.\n{}",
               stderr_str);
    }
}

fn main() {
    let mut args = env::args();

    let _bin = args.next().expect(USAGE);

    let mut input_file = args.next().expect(USAGE);
    let title = args.next().expect(USAGE);
    let resolution = args.next().expect(USAGE);

    // vp9 quality 0-63, recommended 15-35, with 31 recd for 1080p HD
    // See https://developers.google.com/media/vp9/settings/vod/
    let (size, quality) = if &*resolution == "1920" {
        ("1920:1080", 31)
    } else if &*resolution == "1280" {
        ("1280:720", 32)
    } else if &*resolution == "1024" {
        ("1024:576", 35)
    } else if &*resolution == "854" {
        ("854:480", 36)
    } else if &*resolution == "640" {
        ("640:360", 36)
    } else {
        panic!(USAGE)
    };

    // Concat all mp4 files in current directory if inputfile was "."
    if input_file == "." {
        println!("Concatenating...");
        let files = cwd_files_with_extension("mp4");
        cat_files(files, "concatenated.mp4");
        input_file = "concatenated.mp4".to_owned();
    }

    println!("Analyzing loudnorm...");
    let loudnorm = Loudnorm::from_analyze(&input_file);

    println!("Converting video (second pass w/ multiple functions)...");
    convert(&input_file, "tmp1.webm", &loudnorm, size, quality);

    let output_name = title
        .replace("/", "-")
        .replace(" ", "_")
        .to_string();

    println!("Stripping metadata...");
    strip_metadata("tmp1.webm", &*format!("{}.webm", output_name), &title);
}
