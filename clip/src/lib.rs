#![allow(dead_code)]
#![allow(unused_variables)]
// Length (Should be measurd in max MS)
// Primitives: Audio | Midi
// Offset (Right | Left) if we shrink or expand the timeline
//
// |+++++++++++|
// |++++++++|--| Srink Right
// |--|++++++++| Srink Left
// |++++++++++++++| Expand Right We can only expand until the end of the clip, expect if we have a MIDI clip that can be extended indefinitely
// |++++++++|++++++| Expand Left


// AUDIO FORMAT SUPPORTED
//
// MIDI
// AUDIO | MP3 | WAVE | FLAC | AIFF | AIFF-C

// AUDIO ENGINE WORKFLOW
//
// let voice1 = Clip::Audio(ClipOptions);
// let synth = Clip::Midi(ClipOptions);
//
// let clips = [voice1, synth];
//
// let playableBuffer = [];
//
// while true
// {
//
//     play(clips, startPoint);
//
//     for clip in clips {
//         // startPoint >= clip.startPoint & startPoint <= clip.startPoint + offset
//         isPlayable = clip.isPlayable(startPoint)
//         isPlayable ? playableBuffer.push(clip)
//     }
//
//     for buff in playableBuffer {
//     // This operation could be multithreaded in the future
//      clip.play(1 seconds)
//     }
//
//     Advence 1s
//     startPoint += 1000
//
// }

use std::fs::{File, remove_file};

struct Clip {
    file: File,
    options: ClipOptions
}

impl Clip {
    fn new(file: File) -> Result<Self, ClipError> {
    Ok(Self {
            file,
            options: ClipOptions {
                start_point: 0,
                offset_right: 0,
                offset_left: 0,
                length: 0,
            }
        })
    }
}

struct ClipOptions {
    start_point: u64,
    offset_right: u64,
    offset_left: u64,
    length: u64,
}


enum ClipType {
    Audio(Clip),
    Midi(Clip),
}

#[derive(Debug)]
enum ClipError {
    UnsupportedFormat
}

enum ShrinkAction {
    Left,
    Right,
}

enum ShrinkError {
    TooSmall
}


impl ClipType {
    fn is_playable(&self, timepoint: u64) -> bool {
        todo!("implement is_playable")
    }

    fn shrink_clip(&mut self, len: u64, side: ShrinkAction) -> Result<(), ShrinkError> {
        todo!("implement shrink_clip")
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_file() {
        let file = File::create("t_audio.txt").unwrap();
        match Clip::new(file) {
            Err(errType) => panic!("UnsupportedFormat"),
            _ => return
        }
        // remove_file("t_audio.txt");
    }
}
