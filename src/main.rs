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

const CPULIMIT: &'static str = "1200";

const USAGE: &'static str = "USAGE: prepvideo <inputfile> <title> <1920|1280|1024|854|640>";

#[derive(Debug, Clone, Copy)]
struct Vp9Params {
    scale: &'static str,
    target: u32, // in kbps
    crf: u32,
    pass1speed: u32,
    pass2speed: u32,
    tile_columns: u32,
    threads: u32,
}

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
/*
const VP9_PARAMS_320_240: Vp9Params = Vp9Params {
    scale: "320x240",
    target: 150,
    crf: 37,
    pass1speed: 4,
    pass2speed: 1,
    tile_columns: 0,
    threads: 2,
};
*/
const VP9_PARAMS_640_360: Vp9Params = Vp9Params {
    scale: "640x360",
    target: 276,
    crf: 36,
    pass1speed: 4,
    pass2speed: 1,
    tile_columns: 1,
    threads: 4,
};
/*
const VP9_PARAMS_640_480: Vp9Params = Vp9Params {
    scale: "640x480",
    target: 631, // half way between 512(LQ) and 750(MQ)
    crf: 33, // chosen from MQ
    pass1speed: 4,
    pass2speed: 1,
    tile_columns: 1,
    threads: 4,
};
*/
const VP9_PARAMS_854_480: Vp9Params = Vp9Params { // I estimated this one
    scale: "854x480",
    target: 750, // half way between 512(LQ) and 750(MQ)
    crf: 33, // chosen from MQ
    pass1speed: 4,
    pass2speed: 1,
    tile_columns: 1,
    threads: 4,
};
const VP9_PARAMS_1024_576: Vp9Params = Vp9Params { // I estimated this one
    scale: "1024x576",
    target: 820,
    crf: 32,
    pass1speed: 4,
    pass2speed: 2,
    tile_columns: 2,
    threads: 8,
};
const VP9_PARAMS_1280_720: Vp9Params = Vp9Params {
    scale: "1280x720",
    target: 1024,
    crf: 32,
    pass1speed: 4,
    pass2speed: 2,
    tile_columns: 2,
    threads: 8,
};
/*
const VP9_PARAMS_1280_720_60HZ: Vp9Params = Vp9Params {
    scale: "1280x720",
    target: 1800,
    crf: 32,
    pass1speed: 4,
    pass2speed: 2,
    tile_columns: 2,
    threads: 8,
};
*/
const VP9_PARAMS_1920_1080: Vp9Params = Vp9Params {
    scale: "1920x1080",
    target: 1800,
    crf: 31,
    pass1speed: 4,
    pass2speed: 2,
    tile_columns: 2,
    threads: 8,
};
/*
const VP9_PARAMS_1920_1080_60HZ: Vp9Params = Vp9Params {
    scale: "1920x1080",
    target: 3000,
    crf: 31,
    pass1speed: 4,
    pass2speed: 2,
    tile_columns: 2,
    threads: 8,
};
const VP9_PARAMS_2560_1440: Vp9Params = Vp9Params {
    scale: "2560x1440",
    target: 6000,
    crf: 24,
    pass1speed: 4,
    pass2speed: 2,
    tile_columns: 3,
    threads: 16,
};
const VP9_PARAMS_2560_1440_60HZ: Vp9Params = Vp9Params {
    scale: "2560x1440",
    target: 9000,
    crf: 31,
    pass1speed: 4,
    pass2speed: 2,
    tile_columns: 3,
    threads: 16,
};
const VP9_PARAMS_3840_2160: Vp9Params = Vp9Params {
    scale: "3840x2160",
    target: 12000,
    crf: 15,
    pass1speed: 4,
    pass2speed: 2,
    tile_columns: 3,
    threads: 16,
};
const VP9_PARAMS_3840_2160_60HZ: Vp9Params = Vp9Params {
    scale: "3840x2160",
    target: 18000,
    crf: 15,
    pass1speed: 4,
    pass2speed: 2,
    tile_columns: 3,
    threads: 16,
};
 */

fn args_shrink<'a>(command: &mut Command, size: &str) {
    command
        .arg("-vf")
        .arg(&*format!("scale={}", size));
}

fn args_vp9<'a>(command: &mut Command, vp9params: Vp9Params) {
    // We only need 70% of the target Google recommends.
    // I can't see any significant difference.
    // (Our old method was only 66% as many bits, and it looked good to me)
    let bitrate = vp9params.target * 700 / 1000;

    command
        .arg("-b:v").arg(&*format!("{}k", bitrate))
        .arg("-minrate").arg(&*format!("{}k", bitrate * 50 / 100))
        .arg("-maxrate").arg(&*format!("{}k", bitrate * 145 / 100))
        .arg("-tile-columns").arg(&*format!("{}", vp9params.tile_columns))
        .arg("-g").arg("240")        // keyframe spacing
        .arg("-threads").arg(&*format!("{}", vp9params.threads))
        .arg("-quality").arg("good")
        .arg("-crf").arg(&*format!("{}", vp9params.crf))
        .arg("-c:v").arg("libvpx-vp9")
        .arg("-c:a").arg("libopus");
}

fn convert(input: &str, outputfile: &str, loudnorm: &Loudnorm, vp9params: Vp9Params) {
    // Pass 1
    let mut command = Command::new(CPULIMIT_PATH);
    command.arg("-l").arg(CPULIMIT)
        .arg(FFMPEG_PATH)
        .arg("-i").arg(input); // skip loudnorm on pass1
    args_shrink(&mut command, vp9params.scale);
    args_vp9(&mut command, vp9params);
    command.arg("-pass").arg("1")
        .arg("-speed").arg(&*format!("{}", vp9params.pass1speed))
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
    args_shrink(&mut command, vp9params.scale);
    args_vp9(&mut command, vp9params);
    command.arg("-pass").arg("2")
        .arg("-speed").arg(&*format!("{}", vp9params.pass2speed))
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

    let vp9params = if &*resolution == "1920" { VP9_PARAMS_1920_1080 }
    else if &*resolution == "1280" { VP9_PARAMS_1280_720 }
    else if &*resolution == "1024" { VP9_PARAMS_1024_576 }
    else if &*resolution == "854" { VP9_PARAMS_854_480 }
    else if &*resolution == "640" { VP9_PARAMS_640_360 }
    else {  panic!(USAGE) };

    // Concat all mp4 files in current directory if inputfile was "."
    if input_file == "." {
        println!("Concatenating...");
        let files = cwd_files_with_extension("mp4");
        cat_files(files, "concatenated.mp4");
        input_file = "concatenated.mp4".to_owned();
    }

    println!("Analyzing loudnorm...");
    let loudnorm = Loudnorm::from_analyze(&input_file);

    println!("Converting video (two passes w/ multiple functions)...");
    convert(&input_file, "tmp1.webm", &loudnorm, vp9params);

    let output_name = title
        .replace("/", "-")
        .replace(" ", "_")
        .to_string();

    println!("Stripping metadata...");
    strip_metadata("tmp1.webm", &*format!("{}.webm", output_name), &title);
}
