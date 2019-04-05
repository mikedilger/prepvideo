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

const USAGE: &'static str = "USAGE: prepvideo <inputfile> <title>";

fn analyze_loudnorm(input: &str) -> Loudnorm
{
    let output = Command::new(CPULIMIT_PATH).arg("-l").arg(CPULIMIT)
        .arg(FFMPEG_PATH)
        .arg("-y")
        .arg("-i").arg(input)
        .arg("-af")
        .arg(&format!("loudnorm=I={I}:TP={TP}:LRA={LRA}:print_format=json",
                      I=LOUDNORM_LUFS, TP=LOUDNORM_TP, LRA=LOUDNORM_LRA))
        .arg("-f").arg("null").arg("-")
        .output()
        .expect("failed to execute ffmpeg");

    let stderr_str = String::from_utf8_lossy(&*output.stderr).to_string();
    if ! output.status.success() {
        panic!("Failed to run ffmpeg to analyze for loudnorm. Stderr follows.\n{}",
               stderr_str);
    }

    Loudnorm::from_analyze_data(&*stderr_str)
}

fn analyze_vp9(input: &str) {
    let mut command = Command::new(CPULIMIT_PATH);
    command.arg("-l").arg(CPULIMIT)
        .arg(FFMPEG_PATH)
        .arg("-i").arg(input)
        .arg("-c:v").arg("libvpx-vp9")
        .arg("-b:v").arg("2M")
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
        .arg("-b:v").arg("2M")
        .arg("-pass").arg("2")
        .arg("-c:a").arg("libopus")
        .arg("-f").arg("webm");
}

fn args_loudnorm(command: &mut Command, loudnorm: &Loudnorm) {
    command
        .arg("-af")
        .arg(&*format!("loudnorm=I={I}:TP={TP}:LRA={LRA}:measured_I={measured_I}:measured_LRA={measured_LRA}:measured_TP={measured_TP}:measured_thresh={measured_thresh}:offset={offset}:linear=true:print_format=summary",
                       I=LOUDNORM_LUFS,
                       TP=LOUDNORM_TP,
                       LRA=LOUDNORM_LRA,
                       measured_I=loudnorm.input_i,
                       measured_LRA=loudnorm.input_lra,
                       measured_TP=loudnorm.input_tp,
                       measured_thresh=loudnorm.input_thresh,
                       offset=loudnorm.target_offset));
}

fn convert(input: &str, output: &str, loudnorm: &Loudnorm) {
    let mut command = Command::new(CPULIMIT_PATH);
    command.arg("-l").arg(CPULIMIT)
        .arg(FFMPEG_PATH)
        .arg("-y")
        .arg("-i").arg(input);
    args_vp9(&mut command);
    args_shrink(&mut command);
    args_loudnorm(&mut command, loudnorm);

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
    let loudnorm = analyze_loudnorm(&input_file);

    println!("Analyzing conversion (first pass)...");
    analyze_vp9(&input_file);

    println!("Converting video (second pass w/ multiple functions)...");
    convert(&input_file, "stage1.webm", &loudnorm);

    println!("Stripping metadata...");
    strip_metadata("stage1.webm", "stage2.webm", &title);
}
