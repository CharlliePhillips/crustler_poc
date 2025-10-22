use awedio::{backends::CpalBufferSize, sounds::{wrappers::AdjustableSpeed, MemorySound}, *};
use pitch_detection::{detector::{mcleod::McLeodDetector, PitchDetector}, *};
use rppal::gpio::Gpio;
use std::{env, thread::sleep, time::Duration};
use std::fs::File;
use std::io;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use ssd1306::{mode::BufferedGraphicsMode, prelude::*, I2CDisplayInterface, Ssd1306};

const SAMPLE_RATE: usize = 48000;
const SIZE: usize = 1024;
const PADDING: usize = SIZE / 2;
const POWER_THRESHOLD: f64 = 0.0001;
const CLARITY_THRESHOLD: f64 = 0.25;

fn main() {
    init_eq();

    let i2c = rppal::i2c::I2c::new().expect("failed to open I2C bus!");

    // using an alternate address: https://docs.rs/ssd1306/latest/ssd1306/struct.I2CDisplayInterface.html
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(
        interface,
        DisplaySize128x64,
        DisplayRotation::Rotate0,
    ).into_buffered_graphics_mode();
    display.init().unwrap();

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    Text::with_baseline("3", Point::new(8, 8), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();
    sleep(Duration::from_secs(1));
    display.clear_buffer();
    Text::with_baseline("2", Point::new(8, 8), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();

    sleep(Duration::from_secs(1));
    display.clear_buffer();
    Text::with_baseline("1", Point::new(8, 8), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();

    sleep(Duration::from_secs(1));

    let arec= std::process::Command::new("arecord")
        .args(vec!["-D", "plughw:1,0", "-f", "S16_LE", "-c", "1", "-r", "48000", "test_arec.wav"])
        .spawn().expect("Failed to launch arecord!");
    println!("recording...");
    
    display.clear_buffer();
    Text::with_baseline("Recording", Point::new(8, 8), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();

    
    // println!("Press enter to stop recording");
    // let stdin = io::stdin();
    // let input = &mut String::new();
    // let _ = stdin.read_line(input);
    let gpio = Gpio::new().expect("failed to init gpio");
    let input = gpio.get(27).expect("failed to get gpio 27!").into_input();
    
    let mut wait = input.is_high();
    while wait {
        wait = input.is_high();
    }

    // TODO: kill arec in case of sigint failure
    let _ = nix::sys::signal::kill(nix::unistd::Pid::from_raw(arec.id() as i32), nix::sys::signal::Signal::SIGINT);
    
    display.clear_buffer();
    Text::with_baseline("playing", Point::new(8, 8), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();
    println!("playing");


    //let (mut manager, _backend) = awedio::start().expect("couldn't start audio backend!");
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

    let mut next_lev_5: i8 = 11; 
    let mut next_lev_4: i8 = 11; 
    let mut next_lev_3: i8 = 11; 

    for i in 1..37 {
        if next_lev_5 >= 0 {
            set_eq(5, next_lev_5);
            next_lev_5 -= 1;
        }
        if i % 2 == 0 && next_lev_4 >= 0 {
            set_eq(4, next_lev_4);
            next_lev_4 -= 1;
        }
        if i % 2 == 0 && next_lev_3 >= 0{
            set_eq(3, next_lev_3);
            next_lev_3 -= 1;
        }
        sleep(std::time::Duration::from_millis(138));
    }

    // std::thread::sleep(std::time::Duration::from_millis(5000));
    
    display.clear_buffer();
    display.flush().unwrap();
}

fn init_eq() {
    let _amix = std::process::Command::new("amixer")
        .args(vec!["-c", "1", "cset", "numid=9", "on"]);
    for freq in 10..15 {
        let numid_string = format!("numid={}", freq);
        let numid= numid_string.as_str();
        let _amix = std::process::Command::new("amixer")
            .args(vec!["-c", "1", "cset", numid, "12"])
            .spawn().expect("Failed to launch amixer!");
    }
}

fn set_eq(freq: u8, level: i8) {
    if freq > 5 || freq == 0 || level < 0 {
        return;
    }
    let numid_string = format!("numid={}", (freq + 9));
    let numid= numid_string.as_str();
    let lev_string = level.to_string();
    let lev = lev_string.as_str();
    let _amix = std::process::Command::new("amixer")
        .args(vec!["-c", "1", "cset", numid, lev])
        .spawn().expect("Failed to launch amixer!");

}
