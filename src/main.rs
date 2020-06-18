
#[macro_use]
extern crate strum_macros;

use std::io::Read;
use std::process::Command;
use serde::{Serialize, Deserialize};

mod video;
use video::VCodec;

mod audio;
use audio::{ACodec, Loudnorm};

#[derive(Debug, Clone, Copy, PartialEq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(EnumIter, AsRefStr, EnumString)]
pub enum Quality {
    VeryLow,
    Low,
    Medium,
    High,
    VeryHigh,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Operation {
    pub cpulimit: u32,
    pub input: String,
    pub transpose: Option<u8>,
    pub scale: (u16, u16),
    pub loudnorm: bool,
    pub video_quality: Quality,
    pub video_fps: (u32, u32),
    pub video_codec: VCodec,
    pub audio_quality: Quality,
    pub audio_codec: ACodec,
    pub strip_metadata: bool,
    pub title: String,
}

const CPULIMIT_PATH: &'static str = "/usr/bin/cpulimit";
const FFMPEG_PATH: &'static str = "/usr/bin/ffmpeg";

fn main() -> Result<(), Box<dyn std::error::Error>>
{
    println!("Reading operation from stdin...");
    // Read operation from input
    let mut buffer = String::new();
    std::io::stdin().read_to_string(&mut buffer)?;

    // Deserialize as ron
    let operation: Operation = ron::de::from_str(&buffer)?;

    println!("Operation is: {:?}", operation);
    //println!("{}", ron::ser::to_string::<Operation>(&operation)?);

    let title = operation.title
        .replace("/", "-")
        .replace(" ", "_")
        .to_string();
    let output = format!("{}.webm", title);
    let pass1speed = 4;
    let pass2speed = if operation.scale.0 < 1024 { 1 } else { 2 };

    // Analyze loudness
    let loudnorm = if operation.loudnorm {
        Some(Loudnorm::from_analyze(&operation.input, operation.cpulimit))
    } else {
        None
    };

    // Pass 1
    let mut pass1 = build_cmd(&operation, loudnorm.as_ref());
    pass1.arg("-pass").arg("1")
        .arg("-speed").arg(&*format!("{}", pass1speed))
        .arg(&*output);
    let _ = run_cmd(pass1);

    // Pass 2
    let mut pass2 = build_cmd(&operation, loudnorm.as_ref());
    pass2.arg("-pass").arg("2")
        .arg("-speed").arg(&*format!("{}", pass2speed))
        .arg(&*output);
    let _ = run_cmd(pass2);

    Ok(())
}

fn build_cmd(operation: &Operation, loudnorm: Option<&Loudnorm>) -> Command {
    let mut command = Command::new(crate::CPULIMIT_PATH);

    command.arg("-l").arg(&*format!("{}", operation.cpulimit))
        .arg(crate::FFMPEG_PATH)
        .arg("-y")
        .arg("-i").arg(&operation.input);

    let mut audio_filters: Vec<String> = Vec::new();
    let mut video_filters: Vec<String> = Vec::new();

    if operation.loudnorm {
        audio_filters.push(loudnorm.unwrap().convert_af());
    }

    if let Some(t) = operation.transpose {
        video_filters.push(format!("transpose={}",t));
    }

    video_filters.push(format!("scale={}x{}",
                               operation.scale.0,
                               operation.scale.1));

    video_filters.push(format!("fps=fps={}/{}",
                               operation.video_fps.0,
                               operation.video_fps.1));

    if operation.strip_metadata {
        command.arg("-map_metadata").arg("-1")
            .arg("-metadata").arg(format!("title={}",operation.title));
    }

    command.arg("-af").arg(audio_filters.join(","));
    command.arg("-vf").arg(video_filters.join(","));

    match operation.audio_codec {
        ACodec::Copy => {
            command.arg("-c:a").arg("copy");
        },
        ACodec::Opus => {
            audio::opus(&mut command, operation.audio_quality);
        }
    }

    match operation.video_codec {
        VCodec::Copy => {
            command.arg("-c:v").arg("copy");
        },
        _ => {
            video::vp9_or_av1(&mut command, &operation);
        }
    }

    command
}

fn run_cmd(mut command: Command) -> String {
    println!("{:?}", command);

    let output = command.output()
        .expect("failed to execute command");

    let stderr_str = String::from_utf8_lossy(&*output.stderr).to_string();
    if ! output.status.success() {
        panic!("Failed to run ffmpeg multi command.  Stderr follows.\n{}",
               stderr_str);
    }

    stderr_str
}
