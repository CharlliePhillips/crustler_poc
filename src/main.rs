use awedio::{sounds::{wrappers::AdjustableSpeed, MemorySound}, *};
use pitch_detection::{detector::{mcleod::McLeodDetector, PitchDetector}, *};

const SAMPLE_RATE: usize = 48000;
const SIZE: usize = 1024;
const PADDING: usize = SIZE / 2;
const POWER_THRESHOLD: f64 = 0.0001;
const CLARITY_THRESHOLD: f64 = 0.25;

fn main() {
    let (mut manager, _backend) = awedio::start().expect("couldn't start audio backend!");

    let wav_sound = awedio::sounds::open_file("test_voice.wav").expect("couldn't open audio file");
    let mut test_sound = wav_sound.into_memory_sound().expect("Could not make memory sound");
    let sound = test_sound.clone();

    let mut samples: [f64; 1024] = [0.0; 1024];
    for i in 0..1024 {
        samples[i] = match test_sound.next_sample().expect("not enough samples!") {
            NextSample::Sample(s) =>  {
                let test_sample = (s as f64) / 32768.0;
                test_sample
            },
            _ => 0.0
        };
    }
    
    let mut detector = McLeodDetector::new(SIZE, PADDING);

    let pitch = detector
        .get_pitch(&samples, SAMPLE_RATE, POWER_THRESHOLD, CLARITY_THRESHOLD)
        .unwrap(); 

    let correction = 261.6 / (pitch.frequency as f32);

    let base: AdjustableSpeed<MemorySound> = sound.clone().with_adjustable_speed_of(1.0 * correction);
    let second: AdjustableSpeed<MemorySound> = sound.clone().with_adjustable_speed_of(1.26 * correction);
    let third: AdjustableSpeed<MemorySound> = sound.clone().with_adjustable_speed_of(1.498 * correction);

    manager.play(Box::new(base));
    manager.play(Box::new(second));
    manager.play(Box::new(third));

    std::thread::sleep(std::time::Duration::from_millis(2000));
}