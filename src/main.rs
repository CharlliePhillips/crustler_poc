use awedio::{backends::CpalBufferSize, sounds::{wrappers::{AdjustableSpeed, Controllable, Controller, Pausable, Stoppable}, MemorySound}, *};
use pitch_detection::{detector::{mcleod::McLeodDetector, PitchDetector}, *};
use rppal::{gpio::{Event, Gpio, Trigger}, i2c::I2c};
use std::{env, sync::{atomic::{AtomicBool, AtomicU16}, Arc, Mutex}, thread::sleep, time::Duration};
use std::fs::File;
use std::io;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use ssd1306::{mode::BufferedGraphicsMode, prelude::*, I2CDisplayInterface, Ssd1306};
use vl53l1x::{Vl53l1x, Vl53l1xRangeStatus};

enum FilterType {
    HPF,
    LPF,
}

type ROIRight = AtomicBool;

const SAMPLE_RATE: usize = 48000;
const SIZE: usize = 1024;
const PADDING: usize = SIZE / 2;
const POWER_THRESHOLD: f64 = 0.0001;
const CLARITY_THRESHOLD: f64 = 0.25;

const TEMP_PIN: u8 = 27;
const TOF_INT_PIN: u8 = 17;

const DEFAULT_EQ_LEVEL: u16 = 12;

fn main() {
    init_eq();
    let gpio = Gpio::new().expect("failed to init gpio");
    
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
    let input = gpio.get(TEMP_PIN).expect("failed to get gpio 27!").into_input();
    
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
    
    let tof_sensor: Arc<Mutex<Vl53l1x>> = Arc::new(Mutex::new(init_tof()));
    let main_thr_sens = tof_sensor.clone();
    let cur_roi: ROIRight = ROIRight::new(true);
    let cur_eq3: AtomicU16 = AtomicU16::new(DEFAULT_EQ_LEVEL);
    let mut tof_int_pin = gpio.get(TOF_INT_PIN).expect("failed to get tof interrupt pin").into_input();
    tof_int_pin.set_async_interrupt(Trigger::FallingEdge, None, move |e| tof_eq_int(e, tof_sensor.clone(), &cur_roi, &cur_eq3)).expect("failed to setup TOF interrupt");
    let mut sensor = main_thr_sens.lock().expect("failed to lock sensor to begin ranging");
    sensor.start_ranging(vl53l1x::DistanceMode::Short).expect("failed to begin tof ranging");
    drop(sensor);

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

    // let mut base:   (Controllable<Stoppable<AdjustableSpeed<MemorySound>>>, Controller<Stoppable<AdjustableSpeed<MemorySound>>>) = sound.clone().with_adjustable_speed_of((1.0 * correction) as f32).stoppable().controllable();
    // let mut second: (Controllable<Stoppable<AdjustableSpeed<MemorySound>>>, Controller<Stoppable<AdjustableSpeed<MemorySound>>>) = sound.clone().with_adjustable_speed_of((1.26 * correction) as f32).stoppable().controllable();
    // let mut third:  (Controllable<Stoppable<AdjustableSpeed<MemorySound>>>, Controller<Stoppable<AdjustableSpeed<MemorySound>>>) = sound.clone().with_adjustable_speed_of((1.498 * correction) as f32).stoppable().controllable();

    // let (base_snd, mut base_ctrl) = sound.clone().with_adjustable_speed_of((1.0 * correction) as f32).pausable().controllable();
    // let (second_snd, mut second_ctrl) = sound.clone().with_adjustable_speed_of((1.26 * correction) as f32).pausable().controllable();
    // let (third_snd, mut third_ctrl) = sound.clone().with_adjustable_speed_of((1.498 * correction) as f32).pausable().controllable();


    loop {
        let mut base:   (Controllable<Stoppable<AdjustableSpeed<MemorySound>>>, Controller<Stoppable<AdjustableSpeed<MemorySound>>>) = sound.clone().with_adjustable_speed_of((1.0 * correction) as f32).stoppable().controllable();
        let mut second: (Controllable<Stoppable<AdjustableSpeed<MemorySound>>>, Controller<Stoppable<AdjustableSpeed<MemorySound>>>) = sound.clone().with_adjustable_speed_of((1.26 * correction) as f32).stoppable().controllable();
        let mut third:  (Controllable<Stoppable<AdjustableSpeed<MemorySound>>>, Controller<Stoppable<AdjustableSpeed<MemorySound>>>) = sound.clone().with_adjustable_speed_of((1.498 * correction) as f32).stoppable().controllable();
        manager.play(Box::new(base.0));
        manager.play(Box::new(second.0));
        manager.play(Box::new(third.0));

        // let mut next_lev_5: i8 = 11; 
        // let mut next_lev_4: i8 = 11; 
        // let mut next_lev_3: i8 = 11; 

        // for i in 1..37 {
        //     if next_lev_5 >= 0 {
        //         set_eq(5, next_lev_5);
        //         next_lev_5 -= 1;
        //     }
        //     if i % 2 == 0 && next_lev_4 >= 0 {
        //         set_eq(4, next_lev_4);
        //         next_lev_4 -= 1;
        //     }
        //     if i % 2 == 0 && next_lev_3 >= 0{
        //         set_eq(3, next_lev_3);
        //         next_lev_3 -= 1;
        //     }
        //     sleep(std::time::Duration::from_millis(138));
        // }

        std::thread::sleep(std::time::Duration::from_millis(5000));
        base.1.set_stopped();
        second.1.set_stopped();
        third.1.set_stopped();
    }
    display.clear_buffer();
    display.flush().unwrap();
}

fn init_eq() {
    let _amix_en = std::process::Command::new("amixer")
        .args(vec!["-c", "1", "cset", "numid=9", "on"])
        .spawn().expect("Failed to launch amixer!");
    sleep(std::time::Duration::from_millis(50));
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
        .args(vec!["-q", "-c", "1", "cset", numid, lev])
        .output();
    // pray this doesn't cause any issues...

}

fn init_tof() -> Vl53l1x {
    let mut tof_sensor = Vl53l1x::new(1, None).expect("Failed to create TOF sensor struct");
    tof_sensor.soft_reset().expect("Failed to reset TOF sensor");
    tof_sensor.init().expect("Failed to init TOF sensor");
    tof_sensor.set_measurement_timing_budget(20000).expect("failed to set measurement timing");
    tof_sensor.set_inter_measurement_period(24).expect("failed to set inter-measurement timing");

    tof_sensor.set_user_roi(8, 15, 15, 0).expect("failed to set ROI Right");
    
    println!("initilized TOF sensor");
    return tof_sensor;
}

fn tof_eq_int(_event: Event, tof_sensor: Arc<Mutex<Vl53l1x>>, cur_roi: &ROIRight, cur_eq3: &AtomicU16) {
    println!("TOF interrupt");
    let mut sensor = tof_sensor.lock().expect("failed to acquire sensor lock");
    let sample = sensor.read_sample().expect("failed to get right sample");
    println!("sampled: {}mm ({:#?})", sample.distance, sample.status);
    match sample.status {
        Vl53l1xRangeStatus::Ok => {
            let filter_strength: i8 = if sample.distance < 300 {
                (sample.distance/25).try_into().unwrap()
            } else {
                12
            };
            if cur_roi.load(std::sync::atomic::Ordering::SeqCst) {
                set_filter(FilterType::LPF, filter_strength, cur_eq3);
                cur_roi.store(false, std::sync::atomic::Ordering::SeqCst);
                sensor.set_user_roi(0, 0, 7, 15).expect("failed to set ROI Left during interrupt");
            } else {
                set_filter(FilterType::HPF, filter_strength, cur_eq3);
                cur_roi.store(false, std::sync::atomic::Ordering::SeqCst);
                sensor.set_user_roi(0, 0, 7, 15).expect("failed to set ROI Right during interrupt");
            }
        }
        _ => {}
    }
}

fn set_filter(filter: FilterType, strength: i8, cur_eq3: &AtomicU16) {
    match filter {
        FilterType::LPF => {
            set_eq(1, strength/3);
            set_eq(2, strength/2);
        },
        FilterType::HPF => {
            set_eq(4, strength/3);
            set_eq(5, strength/2);

        }
    }

    cur_eq3.fetch_update(std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst, |cur_strength| {
        if cur_strength > (strength) as u16 {
            Some(strength as u16)
        } else {
            Some(cur_strength)
        }
    }).expect("failed to set eq3 strength");

    let eq3: i8 = cur_eq3.load(std::sync::atomic::Ordering::SeqCst).try_into().unwrap(); 
    set_eq(3, eq3);
}
