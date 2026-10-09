use std::time::Instant;

use config::general::GeneralConfig;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};

pub(super) const DEFAULT_AUDIO: &[u8] =
    include_bytes!("../../../../../assets/default-notification-sound.mp3");

pub(super) struct AudioPlayback {
    _handle: MixerDeviceSink,
    player: Player,
    last_time_played: Option<Instant>,
}

impl AudioPlayback {
    pub(super) fn new() -> Self {
        let mut handle = DeviceSinkBuilder::open_default_sink()
            .expect("A system must have at least one default sink");
        handle.log_on_drop(cfg!(debug_assertions));

        Self {
            player: Player::connect_new(handle.mixer()),
            _handle: handle,
            last_time_played: None,
        }
    }

    pub(super) fn is_busy(&self) -> bool {
        !self.player.empty()
    }

    pub(super) fn play_audio<S: Source + Send + 'static>(
        &mut self,
        audio: S,
        general: &GeneralConfig,
    ) {
        if self
            .last_time_played
            .is_some_and(|time| time.elapsed().as_millis() < *general.sound_cooldown as u128)
        {
            return;
        }

        self.player.append(audio);
        self.last_time_played = Some(Instant::now());
    }

    pub(super) fn play_default_audio(&mut self, general: &GeneralConfig) {
        if let Ok(source) = Decoder::try_from(std::io::Cursor::new(DEFAULT_AUDIO)) {
            self.play_audio(source, general);
        };
    }
}

pub(super) fn decode_from_sound_path(
    path: &str,
) -> Option<Decoder<std::io::BufReader<std::fs::File>>> {
    std::fs::File::open(path)
        .ok()
        .and_then(|file| rodio::Decoder::try_from(file).ok())
}

pub(super) fn decode_from_sound_name(
    sound_name: &str,
) -> Option<Decoder<std::io::BufReader<std::fs::File>>> {
    freedesktop_sound::lookup(sound_name)
        .find()
        .and_then(|path| std::fs::File::open(path).ok())
        .and_then(|file| rodio::Decoder::try_from(file).ok())
}
