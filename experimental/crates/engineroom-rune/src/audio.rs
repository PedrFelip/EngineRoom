//! Native playback of the same move sounds used by the web interface.
use std::{io::Cursor, thread};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    Move,
    Capture,
}

/// Selects the destination mainline move when navigation advances.
pub fn forward_sound(
    previous_ply: usize,
    current_ply: usize,
    san: Option<&str>,
    enabled: bool,
) -> Option<Sound> {
    if !enabled || current_ply <= previous_ply {
        return None;
    }
    san.map(|san| {
        if san.contains('x') {
            Sound::Capture
        } else {
            Sound::Move
        }
    })
}

/// Plays asynchronously; unavailable audio devices do not interrupt navigation.
pub fn play(sound: Sound, volume: f32) {
    if !volume.is_finite() || volume <= 0.0 {
        return;
    }
    let volume = volume.clamp(0.0, 1.0);
    thread::spawn(move || {
        let bytes: &'static [u8] = match sound {
            Sound::Move => include_bytes!("../../../../app/public/sounds/Move.mp3"),
            Sound::Capture => include_bytes!("../../../../app/public/sounds/Capture.mp3"),
        };
        let Ok((_stream, handle)) = rodio::OutputStream::try_default() else {
            return;
        };
        let Ok(sink) = rodio::Sink::try_new(&handle) else {
            return;
        };
        let Ok(source) = rodio::Decoder::new(Cursor::new(bytes)) else {
            return;
        };
        sink.set_volume(volume);
        sink.append(source);
        sink.sleep_until_end();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_forward_navigation_plays_destination_move() {
        assert_eq!(
            forward_sound(2, 5, Some("exd5+"), true),
            Some(Sound::Capture)
        );
        assert_eq!(forward_sound(2, 3, Some("O-O"), true), Some(Sound::Move));
        assert_eq!(forward_sound(3, 2, Some("Nf3"), true), None);
        assert_eq!(forward_sound(2, 2, Some("Nf3"), true), None);
        assert_eq!(forward_sound(2, 3, Some("Nf3"), false), None);
        assert_eq!(forward_sound(2, 3, None, true), None);
    }
}
