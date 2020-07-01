
use serde::{Serialize, Deserialize};
use std::process::Command;
use crate::{Quality, Operation};

#[derive(Debug, Clone, Copy, PartialEq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(EnumIter, AsRefStr, EnumString)]
pub enum VCodec {
    Copy,
    Vp9,
    Av1
}

pub fn vp9_or_av1(command: &mut Command, operation: &Operation) {
    let bitrate = {
        let uncompressed_bitrate = uncompressed_bitrate(operation.video_fps,
                                                        operation.scale.0 as u32,
                                                        operation.scale.1 as u32);
        println!("Uncompressed bitrate = {}", uncompressed_bitrate);
        let compression_factor = compression_factor(operation.video_codec,
                                                    operation.video_quality);
        println!("Compression factor = {}", compression_factor);
        (uncompressed_bitrate / compression_factor as u64) as u32
    };
    println!("bitrate = {}", bitrate);

    let tile_columns = if operation.scale.0 < 640 { 0 }
    else if operation.scale.0 < 1024 { 1 }
    else if operation.scale.0 < 2560 { 2 }
    else { 3 };

    let threads = 16; // always reasonable for me
    let crf = 31;     // always reasonable for me

    match operation.video_codec {
        VCodec::Copy => { },
        VCodec::Vp9 => {
            command
                .arg("-c:v").arg("libvpx-vp9")
                .arg("-quality").arg("good");
        },
        VCodec::Av1 => {
            command
                .arg("-c:v").arg("libaom-av1")
                .arg("-strict").arg("-2");
        },
    }

    command
        .arg("-b:v").arg(&*format!("{}", bitrate))
        .arg("-minrate").arg(&*format!("{}", bitrate * 50 / 100))
        .arg("-maxrate").arg(&*format!("{}", bitrate * 145 / 100))
        .arg("-tile-columns").arg(&*format!("{}", tile_columns))
        .arg("-g").arg("240")        // keyframe spacing
        .arg("-threads").arg(&*format!("{}", threads))
        .arg("-crf").arg(&*format!("{}", crf));
}


fn uncompressed_bitrate(fps: (u32, u32), x: u32, y: u32) -> u64 {
    // 24 from bits per pixel (RGB 8-bit)
    24 * x as u64 * y as u64 * fps.0 as u64 / fps.1 as u64
}

fn compression_factor(codec: VCodec, quality: Quality) -> u32 {
    let factor = match quality {
        Quality::VeryLow => 4600,
        Quality::Low => 3200, // Fast moving stuff (eye blinks) look a bit wrong, but otherwise looks ok
        Quality::Medium => 1500, // I cannot tell the difference between this and higher quality
        Quality::High => 640, // 640 is near average of google recommendations
        Quality::VeryHigh => 280, // 280 is better than almost all of google recommendations
    };
    match codec {
        VCodec::Copy => factor,
        VCodec::Vp9 => factor,
        VCodec::Av1 => factor * 100 / 70, // 30% less bits needed for AV1
    }
}

/* We no longer get fps.  We instead convert to a target fps during conversion
const FFPROBE_PATH: &'static str = "/usr/bin/ffprobe";
pub fn get_fps(input: &str) -> (u32, u32) {
    let mut command = Command::new(FFPROBE_PATH);
    command.arg("-v").arg("0")
        .arg("-of").arg("csv=p=0")
        .arg("-select_streams").arg("v:0")
        .arg("-show_entries").arg("stream=r_frame_rate")
        .arg(input);

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
            return (30,1);
        },
        Some(i) => {
            let numerator = &fpsstr[..i].parse::<u32>().unwrap();
            let denominator = &fpsstr[i+1..].parse::<u32>().unwrap();
            return (*numerator, *denominator)
        }
    }
}
*/

