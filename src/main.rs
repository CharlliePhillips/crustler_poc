use awedio::{sounds::{wrappers::AdjustableSpeed, MemorySound}, *};
use pitch_detection::{detector::{mcleod::McLeodDetector, PitchDetector}, *};
use std::{env, thread::sleep, time::Duration};
use std::fs::File;
use std::io;
use std::io::BufWriter;
extern crate hound;
extern crate crossbeam;

const SAMPLE_RATE: usize = 48000;
const SIZE: usize = 1024;
const PADDING: usize = SIZE / 2;
const POWER_THRESHOLD: f64 = 0.0001;
const CLARITY_THRESHOLD: f64 = 0.25;

fn main() {

    
    // match record("test.wav") {
    //     Err(x) => {
    //         println!("Error: {}", x);
    //         return;
    //     },
    //     _ => {}
    // }

    let arec= std::process::Command::new("arecord")
        .args(vec!["-f", "S16_LE", "-c", "1", "-r", "48000", "test_arec.wav"])
        .spawn().expect("Failed to launch arecord!");

    
    println!("Press enter to stop recording");
    let stdin = io::stdin();
    let input = &mut String::new();
    let _ = stdin.read_line(input);
    
    // TODO: kill arec in case of sigint failure
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(arec.id() as i32), nix::sys::signal::Signal::SIGINT);


    let (mut manager, _backend) = awedio::start().expect("couldn't start audio backend!");

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

    let correction = 261.6 / (pitch.frequency as f32);

    let base: AdjustableSpeed<MemorySound> = sound.clone().with_adjustable_speed_of(1.0 * correction);
    let second: AdjustableSpeed<MemorySound> = sound.clone().with_adjustable_speed_of(1.26 * correction);
    let third: AdjustableSpeed<MemorySound> = sound.clone().with_adjustable_speed_of(1.498 * correction);

    manager.play(Box::new(base));
    manager.play(Box::new(second));
    manager.play(Box::new(third));

    std::thread::sleep(std::time::Duration::from_millis(2000));
}

struct WavRecorder {
    writer: hound::WavWriter<BufWriter<File>>,
}

impl WavRecorder {
    fn read_callback(&mut self, stream: &mut soundio::InStreamReader) {
        let mut frames_left = stream.frame_count_max();

        // libsoundio reads samples in chunks, so we need to loop until there's nothing to read.
        loop {
            if let Err(e) = stream.begin_read(frames_left) {
                println!("Error reading from stream: {}", e);
                return;
            }
            for f in 0..stream.frame_count() {
                for c in 0..stream.channel_count() {
                    // In reality you shouldn't write to disk in the callback, but have some buffer instead.
                    match self.writer.write_sample(stream.sample::<i16>(c, f)) {
                        Ok(_) => {}
                        Err(e) => println!("Error: {}", e),
                    }
                }
            }

            frames_left -= stream.frame_count();
            if frames_left <= 0 {
                break;
            }

            stream.end_read();
        }
    }
}

// Print sound soundio debug info and record a sound.
fn record(filename: &str) -> Result<(), String> {
    // TODO: Probe which channels/sample rates are available.
    let channels = 1;
    let sample_rate = 48000;

    let spec = hound::WavSpec {
        channels: channels,
        sample_rate: sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    // Try to open the output file.
    let writer = hound::WavWriter::create(filename, spec).map_err(|x| x.to_string())?;

    println!("Soundio version: {}", soundio::version_string());

    let mut ctx = soundio::Context::new();
    ctx.set_app_name("Recorder");
    ctx.connect()?;

    println!("Current backend: {:?}", ctx.current_backend());

    // We have to flush events so we can scan devices.
    ctx.flush_events();
    // I guess these are always signed little endian?
    let soundio_format = soundio::Format::S16LE;

    let default_layout = soundio::ChannelLayout::get_default(channels as _);
    println!(
        "Default layout for {} channel(s): {:?}",
        channels, default_layout
    );

    let input_dev = ctx
        .default_input_device()
        .map_err(|_| "Error getting default input device".to_string())?;

    println!(
        "Default input device: {} {}",
        input_dev.name(),
        if input_dev.is_raw() { "raw" } else { "cooked" }
    );

    let mut recorder = WavRecorder { writer: writer };

    println!("Opening default input stream");
    let mut input_stream = input_dev.open_instream(
        sample_rate as _,
        soundio_format,
        default_layout,
        0.1,
        |x| recorder.read_callback(x),
        None::<fn()>,
        None::<fn(soundio::Error)>,
    )?;


    println!("recording in\n3");
    sleep(Duration::from_secs(1));
    println!("2");
    sleep(Duration::from_secs(1));
    println!("1");
    sleep(Duration::from_secs(1));
    
    println!("Starting stream");
    input_stream.start()?;

    // Wait for the user to press a key.
    println!("Press enter to stop recording");
    let stdin = io::stdin();
    let input = &mut String::new();
    let _ = stdin.read_line(input);

    Ok(())
}
