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
const FFPROBE_PATH: &'static str = "/usr/bin/ffprobe";

const CPULIMIT: &'static str = "1200";

const USAGE: &'static str = "USAGE: prepvideo <inputfile> <title> <x-resolution> <vp9|av1> <VHQ|HQ|MQ|LQ|VLQ>";

// AS A REFERENCE POINT, my Nexus 5x phone video comes out in H.264 at 1920x1200 @30hz
// w/ a bit rate of 16,995 kbps.  That's 9.44x times as many bits as google's VOD recommendation
// for that sized video.
//
// chaturbate.com videos of 1920x1200@30fps H.264 are between 4900k and 5100k.
// That is 2.7x as many bits as google's VOD recommendation for that sized video.
//
// Previous versions of prepvideo did single-pass and Q mode (-b:v 0).  This holds quality
// more strictly constant... but I didn't pass in a bitrate at all (min/max/target), just
// the -crf value... and files were smaller than they are now.  So Google's VOD specs are pretty
// high specs, producing about 1.6x larger files.

// Google recommendations for Video on Demand (file based):
// https://web.archive.org/web/20200117200622/https://developers.google.com/media/vp9/settings/vod/
// https://developers.google.com/media/vp9/settings/vod/

fn main() {
    let mut args = env::args();

    let _bin = args.next().expect(USAGE);

    let mut input_file = args.next().expect(USAGE);
    let title = args.next().expect(USAGE);
    let xres = args.next().expect(USAGE).parse::<i32>().unwrap();
    let algo = args.next().expect(USAGE);
    let level = args.next().expect(USAGE);

    // Concat all mp4 files in current directory if inputfile was "."
    if input_file == "." {
        println!("Concatenating...");
        let files = cwd_files_with_extension("mp4");
        cat_files(files, "concatenated.mp4");
        input_file = "concatenated.mp4".to_owned();
    }

    println!("Extracting FPS...");
    let fps = get_fps(&input_file);

    println!("Analyzing loudnorm...");
    let loudnorm = Loudnorm::from_analyze(&input_file);

    println!("Converting video (two passes w/ multiple functions)...");
    convert(&input_file, "tmp1.webm", &loudnorm, xres, fps, &*algo, &*level);

    let output_name = title
        .replace("/", "-")
        .replace(" ", "_")
        .to_string();

    println!("Stripping metadata...");
    strip_metadata("tmp1.webm", &*format!("{}.webm", output_name), &title);
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

fn get_fps(input: &str) -> i32 {
    let mut command = Command::new(FFPROBE_PATH);
    command.arg("-v").arg("0")
        .arg("-of").arg("csv=p=0")
        .arg("-select_streams").arg("v:0")
        .arg("-show_entries").arg("stream=r_frame_rate")
        .arg(input);
    println!("{:?}", command);
    let output = command
        .output()
        .expect("failed to execute ffprobe");

    if ! output.status.success() {
        let stderr_str = String::from_utf8_lossy(&*output.stderr).to_string();
        panic!("Failed to run ffprobe.  Stderr follows.\n{}",
               stderr_str);
    }

    let fpsstr = String::from_utf8_lossy(&*output.stdout).trim().to_string();
    match fpsstr.find('/') {
        None => {
            println!("Could not determine FPS, using 30.");
            return 30;
        },
        Some(i) => {
            let numerator = &fpsstr[..i].parse::<i32>().unwrap();
            let denominator = &fpsstr[i+1..].parse::<i32>().unwrap();
            return (*numerator as f32 / *denominator as f32).round() as i32;
        }
    }
}

fn convert(input: &str, outputfile: &str, loudnorm: &Loudnorm, xres: i32, fps: i32,
           algo: &str, level: &str)
{

    // We always use 1:1.777777 aspect ratios
    let yres = xres * 5625 / 10000;

    let pass1speed = 4; // at all resolutions
    let pass2speed = if xres < 1024 { 1 } else { 2 };

    // Pass 1
    let mut command = Command::new(CPULIMIT_PATH);
    command.arg("-l").arg(CPULIMIT)
        .arg(FFMPEG_PATH)
        .arg("-i").arg(input); // skip loudnorm on pass1
    args_shrink(&mut command, &*format!("{}x{}", xres, yres));
    args_video(&mut command, xres, yres, fps, algo, level);
    args_audio(&mut command, level);
    command.arg("-pass").arg("1")
        .arg("-speed").arg(&*format!("{}", pass1speed))
        .arg("-y");
    command.arg(outputfile);

    println!("{:?}", command);

    let output = command
        .output()
        .expect("failed to execute ffmpeg");

    if ! output.status.success() {
        let stderr_str = String::from_utf8_lossy(&*output.stderr).to_string();
        panic!("Failed to run ffmpeg multi command.  Stderr follows.\n{}",
               stderr_str);
    }

    // Pass 2
    let mut command = Command::new(CPULIMIT_PATH);
    command.arg("-l").arg(CPULIMIT)
        .arg(FFMPEG_PATH)
        .arg("-i").arg(input)
        .arg("-af")
        .arg(&*loudnorm.convert_af());
    args_shrink(&mut command, &*format!("{}x{}", xres, yres));
    args_video(&mut command, xres, yres, fps, algo, level);
    args_audio(&mut command, level);
    command.arg("-pass").arg("2")
        .arg("-speed").arg(&*format!("{}", pass2speed))
        .arg("-y");
    command.arg(outputfile);

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

fn args_audio(command: &mut Command, level: &str) {
    // For my voice, in stereo, 32kbps is plenty
    // For streaming music, 64-96 kbps.
    // For music storage
    //    stereo: 96-128 kbps
    //    5.1: 128-256 kbps
    //    7.1: 256-450 kbps
    // For archival
    //    use FLAC to prevent generation loss

    let bitrate = match level {
        "VHQ" => 96,
        "HQ" => 64,
        "MQ" => 32,
        "LQ" => 24,
        "VLQ" => 16,
        _ => 64,
    };
    command
        .arg("-c:a").arg("libopus")
        .arg("-b:a").arg(&*format!("{}k",bitrate));
}

fn args_shrink(command: &mut Command, size: &str) {
    command
        .arg("-vf")
        .arg(&*format!("scale={}", size));
}

fn args_video(command: &mut Command, xres: i32, yres: i32, fps: i32, algo: &str, level: &str) {

    let compression_factor = match level {
        "VHQ" => 280, // 280 is better than almost all of google recommendations
        "HQ" => 640, // 640 is near average of google recommendations
        "MQ" => 1500, // I cannot tell the difference between this and "google"
        "LQ" => 3200, // Fast moving stuff (eye blinks) look a bit wrong, but otherwise looks ok
        "VLQ" => 4600,
        _ => 1500,
    };
    let compression_factor = match algo {
        "vp9" => compression_factor,
        "av1" => compression_factor * 100 / 70, // 30% less bits, 70% remaining bits
        _ => panic!(USAGE)
    };

    let uncompressed_bitrate = 24 * fps * xres * yres;

    let bitrate = uncompressed_bitrate / compression_factor;
    println!("Target bitrate will be {}kbps", bitrate / 1000);

    let tile_columns = if xres < 640 { 0 }
    else if xres < 1024 { 1 }
    else if xres < 2560 { 2 }
    else { 3 };

    let threads = 16; // for me, always use 16 threads
    let crf = 31; // 31 is always reasonable

    command
        .arg("-b:v").arg(&*format!("{}", bitrate))
        .arg("-minrate").arg(&*format!("{}", bitrate * 50 / 100))
        .arg("-maxrate").arg(&*format!("{}", bitrate * 145 / 100))
        .arg("-tile-columns").arg(&*format!("{}", tile_columns))
        .arg("-g").arg("240")        // keyframe spacing
        .arg("-threads").arg(&*format!("{}", threads))
        .arg("-crf").arg(&*format!("{}", crf));

    match algo {
        "vp9" => {
            command
                .arg("-quality").arg("good")
                .arg("-c:v").arg("libvpx-vp9");
        },
        "av1" => {
            command
                .arg("-c:v").arg("libaom-av1")
                .arg("-strict").arg("-2");
        },
        _ => panic!(USAGE)
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
