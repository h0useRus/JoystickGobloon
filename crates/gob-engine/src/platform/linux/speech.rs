//! Linux speech through speech-dispatcher's `spd-say` client (packaged with speech-dispatcher
//! on the usual distributions; no library needed at build time). speech-dispatcher queues
//! what is said; "interrupt" cancels what is spoken first (`spd-say -C`, which also stops
//! other programs' speech, e.g. a screen reader's).

use std::process::{Command, Stdio};

use crate::media::{SpeechQueue, SpeechRequest};

const SPD_SAY: &str = "spd-say";

pub(crate) struct Speech;

impl crate::platform::SpeechEngine for Speech {
    fn new() -> Option<Self> {
        let found = Command::new(SPD_SAY)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if !found {
            log::warn!("text-to-speech needs speech-dispatcher ({SPD_SAY} not found)");
        }
        found.then_some(Self)
    }

    fn voices() -> Vec<String> {
        match Command::new(SPD_SAY).arg("-L").output() {
            Ok(o) if o.status.success() => parse_voices(&String::from_utf8_lossy(&o.stdout)),
            _ => Vec::new(),
        }
    }

    fn speak(&mut self, request: &SpeechRequest) {
        if request.queue == SpeechQueue::Interrupt
            && let Err(e) = Command::new(SPD_SAY).arg("-C").status()
        {
            log::error!("{SPD_SAY} -C: {e}");
        }
        // spd-say returns once speech-dispatcher has the message: speech stays asynchronous.
        let spawned = Command::new(SPD_SAY)
            .args(args(request))
            .stdout(Stdio::null())
            .spawn()
            .and_then(|mut child| child.wait());
        if let Err(e) = spawned {
            log::error!("text-to-speech: {e}");
        }
    }
}

/// -1..1 -> spd-say's -100..100.
fn percent(v: f32) -> String {
    ((v.clamp(-1.0, 1.0) * 100.0).round() as i32).to_string()
}

fn args(r: &SpeechRequest) -> Vec<String> {
    let mut args = vec![
        "-r".into(),
        percent(r.rate),
        "-p".into(),
        percent(r.pitch),
        // Volume 0..1 -> -100..100.
        "-i".into(),
        percent(r.volume.clamp(0.0, 1.0) * 2.0 - 1.0),
    ];
    if let Some(voice) = r.voice.as_ref().filter(|v| !v.is_empty()) {
        args.extend(["-y".into(), voice.clone()]);
    }
    // Text after "--", so a leading "-" is not read as an option.
    args.extend(["--".into(), r.text.clone()]);
    args
}

/// `spd-say -L`: a header line, then "NAME LANGUAGE VARIANT" per voice in padded columns;
/// names may contain spaces, the last two columns do not.
fn parse_voices(text: &str) -> Vec<String> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let mut words: Vec<&str> = line.split_whitespace().collect();
            if words.len() < 3 {
                return None;
            }
            words.truncate(words.len() - 2);
            Some(words.join(" "))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_and_voices() {
        let r = SpeechRequest {
            text: "-Mode combat".into(),
            voice: Some("English (America)".into()),
            rate: 0.5,
            volume: 0.8,
            pitch: -1.0,
            queue: SpeechQueue::Back,
        };
        assert_eq!(
            args(&r),
            [
                "-r",
                "50",
                "-p",
                "-100",
                "-i",
                "60",
                "-y",
                "English (America)",
                "--",
                "-Mode combat"
            ]
        );
        let listing = "                   NAME     LANGUAGE      VARIANT\n\
                       English (America)        en-US         none\n\
                       Russian                  ru            none\n";
        assert_eq!(parse_voices(listing), ["English (America)", "Russian"]);
    }
}
