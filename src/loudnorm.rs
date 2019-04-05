
use crate::{CPULIMIT_PATH, CPULIMIT, FFMPEG_PATH};
use regex::Regex;
use std::process::Command;

/// LUFS should be between -20 (minimum) and -16 (maximum).
///    -20 gives the greatest dynamic range and the least processing.
///    -16 gives the most loudness.
pub const LOUDNORM_LUFS: &'static str = "-19";

/// TP (limiter threshold peak) is the level of the true peak.  This is recommended to -1.0
/// so as not to clip, or some do -1.5.  Don't do 0.
pub const LOUDNORM_TP: &'static str = "-1.0";

/// LRA is the variation in loudness on a macroscopic scale.  Default is 7.
/// Other references tend to use 11.
pub const LOUDNORM_LRA: &'static str = "9";

#[derive(Debug)]
pub struct Loudnorm {
    /// Measured input_i
    pub input_i: String,

    /// Measured input_lra
    pub input_lra: String,

    /// Measured input_tp
    pub input_tp: String,

    /// Measured input_thresh
    pub input_thresh: String,

    /// Measured target_offset
    pub target_offset: String,
}

impl Loudnorm {

    pub fn analyze_af() -> String {
        format!("loudnorm=I={I}:TP={TP}:LRA={LRA}:print_format=json",
                I=LOUDNORM_LUFS, TP=LOUDNORM_TP, LRA=LOUDNORM_LRA)
    }

    pub fn convert_af(&self) -> String {
        format!("loudnorm=I={I}:TP={TP}:LRA={LRA}:measured_I={measured_I}:measured_LRA={measured_LRA}:measured_TP={measured_TP}:measured_thresh={measured_thresh}:offset={offset}:linear=true:print_format=summary",
                I=LOUDNORM_LUFS,
                TP=LOUDNORM_TP,
                LRA=LOUDNORM_LRA,
                measured_I=self.input_i,
                measured_LRA=self.input_lra,
                measured_TP=self.input_tp,
                measured_thresh=self.input_thresh,
                offset=self.target_offset)
    }

    pub fn from_analyze(input_file: &str) -> Loudnorm {
        let mut command = Command::new(CPULIMIT_PATH);
        command.arg("-l").arg(CPULIMIT)
            .arg(FFMPEG_PATH)
            .arg("-y")
            .arg("-i").arg(input_file)
            .arg("-af")
            .arg(&*Loudnorm::analyze_af())
            .arg("-f").arg("null").arg("-");

        println!("{:?}", command);

        let output = command.output()
        .expect("failed to execute ffmpeg");

        let stderr_str = String::from_utf8_lossy(&*output.stderr).to_string();
        if ! output.status.success() {
            panic!("Failed to run ffmpeg to analyze for loudnorm. Stderr follows.\n{}",
                   stderr_str);
        }

        Loudnorm::from_analyze_data(&*stderr_str)
    }

    pub fn from_analyze_data(data: &str) -> Loudnorm {
        let mut loudnorm = Loudnorm {
            input_i: "".to_string(),
            input_lra: "".to_string(),
            input_tp: "".to_string(),
            input_thresh: "".to_string(),
            target_offset: "".to_string(),
        };

        let input_i_re = Regex::new(r##""input_i" : "(-?\d+.\d+)""##).unwrap();
        for cap in input_i_re.captures_iter(data) {
            loudnorm.input_i = cap[1].to_owned();
        }
        if loudnorm.input_i.is_empty() {
            panic!("Did not find input_i");
        }

        let input_lra_re = Regex::new(r##""input_lra" : "(-?\d+.\d+)""##).unwrap();
        for cap in input_lra_re.captures_iter(data) {
            loudnorm.input_lra = cap[1].to_owned();
        }
        if loudnorm.input_lra.is_empty() {
            panic!("Did not find input_lra");
        }

        let input_tp_re = Regex::new(r##""input_tp" : "(-?\d+.\d+)""##).unwrap();
        for cap in input_tp_re.captures_iter(data) {
            loudnorm.input_tp = cap[1].to_owned();
        }
        if loudnorm.input_tp.is_empty() {
            panic!("Did not find input_tp");
        }

        let input_thresh_re = Regex::new(r##""input_thresh" : "(-?\d+.\d+)""##).unwrap();
        for cap in input_thresh_re.captures_iter(data) {
            loudnorm.input_thresh = cap[1].to_owned();
        }
        if loudnorm.input_thresh.is_empty() {
            panic!("Did not find input_thresh");
        }

        let target_offset_re = Regex::new(r##""target_offset" : "(-?\d+.\d+)""##).unwrap();
        for cap in target_offset_re.captures_iter(data) {
            loudnorm.target_offset = cap[1].to_owned();
        }
        if loudnorm.target_offset.is_empty() {
            panic!("Did not find target_offset");
        }

        println!("LOUDNORM DATA IS: {:?}", loudnorm);

        loudnorm
    }
}
