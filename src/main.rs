use awedio::{backends::CpalBufferSize, sounds::{wrappers::AdjustableSpeed, MemorySound}, *};
use pitch_detection::{detector::{mcleod::McLeodDetector, PitchDetector}, *};
use std::{env, thread::sleep, time::Duration};
use std::fs::File;
use std::io;

const SAMPLE_RATE: usize = 48000;
const SIZE: usize = 1024;
const PADDING: usize = SIZE / 2;
const POWER_THRESHOLD: f64 = 0.0001;
const CLARITY_THRESHOLD: f64 = 0.25;

fn main() {
    let arec= std::process::Command::new("arecord")
        .args(vec!["-D", "plughw:1,0", "-f", "S16_LE", "-c", "1", "-r", "48000", "test_arec.wav"])
        .spawn().expect("Failed to launch arecord!");

    
    println!("Press enter to stop recording");
    let stdin = io::stdin();
    let input = &mut String::new();
    let _ = stdin.read_line(input);
    
    // TODO: kill arec in case of sigint failure
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(arec.id() as i32), nix::sys::signal::Signal::SIGINT);


    let (mut manager, _backend) = awedio::start().expect("couldn't start audio backend!");
    let mut backend =
        backends::CpalBackend::with_default_host_and_device(1,48000,CpalBufferSize::Default).ok_or(backends::CpalBackendError::NoDevice).expect("failed to initilize cpal backend!");
    let mut manager = backend.start(|error| eprintln!("error with cpal output stream: {}", error)).expect("failed to initialize sound manager!");

    let wav_sound = awedio::sounds::open_file("test_arec.wav").expect("couldn't open audio file");
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

    let correction: f64 = (523.2 / (pitch.frequency as f64));

    let base: AdjustableSpeed<MemorySound> = sound.clone().with_adjustable_speed_of((1.0 * correction) as f32);
    let second: AdjustableSpeed<MemorySound> = sound.clone().with_adjustable_speed_of((1.26 * correction) as f32);
    let third: AdjustableSpeed<MemorySound> = sound.clone().with_adjustable_speed_of((1.498 * correction) as f32);

    manager.play(Box::new(base));
    manager.play(Box::new(second));
    manager.play(Box::new(third));

    std::thread::sleep(std::time::Duration::from_millis(5000));
}
