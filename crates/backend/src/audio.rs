use rodio::OutputStreamHandle;
use rodio::{source::Source, Decoder, OutputStream};
use std::fs::File;
use std::io::BufReader;

const SOUND: &str = "/home/hapka/Documents/kits/ KEHMICS - ESSENTIALS STASH 2024/ KEHMICS - ESSENTIALS STASH 2024/ (no skin) !! Essentials Stash 2024 LE (limited Edition) 「@Khemics_」/Zephyr - Oneshots Essentials [ESS24]/Bells/(C) [ESS24] KYROGEN BELL @khemics_.wav";

pub async fn play_sound(stream_handle: &OutputStreamHandle) {
    let sink = rodio::Sink::try_new(stream_handle).unwrap();

    let file = BufReader::new(File::open(SOUND).unwrap());
    let source = Decoder::new(file).unwrap();

    sink.append(source);
    sink.detach();
}
