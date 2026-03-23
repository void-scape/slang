#![feature(bool_to_result)]
#![feature(uint_bit_width)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use crate::stage::Stage;
use bevy_app::prelude::*;
use bevy_ecs::component::Component;
use std::{io::IsTerminal, time::Instant};

mod error;
mod stage;
mod tree;

#[derive(Default, Clone, Component)]
pub struct Flags {
    pub log: bool,
    pub run: bool,
    pub capture: bool,
    pub codegen: bool,
    pub input: Vec<String>,
    pub output: Option<String>,
}

impl Flags {
    pub fn output(&self) -> String {
        self.output.clone().unwrap_or_else(|| String::from("a.out"))
    }
}

#[derive(Component)]
pub struct Timing {
    now: Instant,
    markers: Vec<(Stage, f32)>,
}

impl Timing {
    pub fn finished(&mut self, stage: Stage) {
        let dur = self.now.elapsed().as_secs_f32();
        self.markers.push((stage, dur));
        self.now = Instant::now();
    }

    pub fn report(&self) {
        let total = self.markers.iter().fold(0.0, |total, (_, t)| total + t);
        if std::io::stdout().is_terminal() {
            println!("\x1b[92m\x1b[1mCompiled\x1b[0m in {total:.4}s");
            for (stage, dur) in self.markers.iter() {
                println!(
                    "\x1b[38;5;246m... {:<10} {dur:.4}\x1b[0m",
                    format!("{stage:?}")
                );
            }
        } else {
            println!("Compiled in {total:.4}s");
            for (stage, dur) in self.markers.iter() {
                println!("... {:<10} {dur:.4}", format!("{stage:?}"));
            }
        }
    }
}

pub fn compile(flags: Flags) -> AppExit {
    let mut app = App::new();
    if flags.log {
        app.add_plugins(bevy_log::LogPlugin::default());
    }
    // NOTE: The first state runs before PreStartup, so this needs to be
    // spawned here.
    app.world_mut().spawn((
        flags,
        Timing {
            now: Instant::now(),
            markers: Vec::new(),
        },
    ));
    app.add_plugins((stage::plugin, error::plugin))
        .set_runner(|mut app| {
            loop {
                app.update();
                if let Some(exit) = app.should_exit() {
                    return exit;
                }
            }
        })
        .run()
}

#[cfg(test)]
mod test {
    include!(concat!(env!("OUT_DIR"), "/slang_tests.rs"));
    fn compile_and_run(input: &str, capture_path: &str) {
        let record_capture = std::env::var("CAPTURE").unwrap_or_default() == "1";
        let capture = std::fs::read(capture_path).ok();
        let path = format!("/tmp/slexec_{}", input.replace('/', "_"));
        assert!(
            crate::compile(crate::Flags {
                input: vec![input.to_string()],
                output: Some(path.clone()),
                ..Default::default()
            })
            .is_success()
        );
        let result = std::process::Command::new(path).output().unwrap();
        if record_capture {
            std::fs::write(capture_path, &result.stdout).unwrap();
            return;
        }
        println!("{}", String::try_from(result.stdout.clone()).unwrap());
        assert_eq!(result.status.code(), Some(0));
        if let Some(capture) = capture
            && capture != result.stdout
        {
            panic!("stdout differs");
        }
    }
}
