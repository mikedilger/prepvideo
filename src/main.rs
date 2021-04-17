// © Copyright 2021 Michael Dilger <mike@mikedilger.com>
// All rights reserved.

#[macro_use]
extern crate strum_macros;

use std::io::{Read, Write};
use std::fs::File;
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

#[derive(Debug, Clone, Copy, PartialEq, Hash)]
#[derive(Serialize, Deserialize)]
#[derive(EnumIter, AsRefStr, EnumString)]
pub enum Container {
    Mp4,
    Mkv,
    Webm
}
impl Container {
    pub fn extension(&self) -> &'static str {
        match *self {
            Container::Mp4 => "mp4",
            Container::Mkv => "mkv",
            Container::Webm => "webm",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Operation {
    pub cpulimit: u32,
    pub inputs: Vec<String>,
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
    pub container: Container,
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

    // concatenation of inputs
    {
        let mut concat_list_file = File::create("concat.txt")?;
        for input in &operation.inputs {
            writeln!(concat_list_file, "file '{}'", input)?;
        }
        let mut cmd = Command::new(crate::FFMPEG_PATH);
        cmd.arg("-f").arg("concat")
            .arg("-i").arg("concat.txt")
            .arg("-c").arg("copy")
            .arg("concat.mp4");
        let _ = run_cmd(cmd);
    }

    let title = operation.title
        .replace("/", "-")
        .replace(" ", "_")
        .to_string();
    let output = format!("{}.{}", title, operation.container.extension());
    let pass1speed = 4;
    let pass2speed = if operation.scale.0 < 1024 { 1 } else { 2 };

    // Analyze loudness
    let loudnorm = if operation.loudnorm {
        Some(Loudnorm::from_analyze("concat.mp4", operation.cpulimit))
    } else {
        None
    };

    // Pass 1
    let mut pass1 = build_cmd(&operation, loudnorm.as_ref(), "concat.mp4");
    pass1.arg("-pass").arg("1")
        .arg("-speed").arg(&*format!("{}", pass1speed))
        .arg(&*output);
    let _ = run_cmd(pass1);

    // Pass 2
    let mut pass2 = build_cmd(&operation, loudnorm.as_ref(), "concat.mp4");
    pass2.arg("-pass").arg("2")
        .arg("-speed").arg(&*format!("{}", pass2speed))
        .arg(&*output);
    let _ = run_cmd(pass2);

    Ok(())
}

fn build_cmd(operation: &Operation, loudnorm: Option<&Loudnorm>,
             concat_file: &str) -> Command {
    let mut command = Command::new(crate::CPULIMIT_PATH);

    command.arg("-l").arg(&*format!("{}", operation.cpulimit))
        .arg(crate::FFMPEG_PATH)
        .arg("-y")
        .arg("-i").arg(concat_file);

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

    if operation.audio_codec != ACodec::Copy {
        let af = audio_filters.join(",");
        if af.len() > 0 {
            command.arg("-af").arg(af);
        }
    }

    if operation.video_codec != VCodec::Copy {
        let vf = video_filters.join(",");
        if vf.len() > 0 {
            command.arg("-vf").arg(vf);
        }
    }

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
