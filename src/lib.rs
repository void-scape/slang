#![feature(bool_to_result)]
#![feature(uint_bit_width)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use bevy_app::prelude::*;

mod error;
mod stage;
mod tree;

#[derive(Default)]
pub struct Config {
    pub log: bool,
    pub run: bool,
    pub capture: bool,
}

pub fn compile(files: Vec<String>, config: Config) {
    let mut app = App::new();
    if config.log {
        app.add_plugins(bevy_log::LogPlugin::default());
    }
    app.add_plugins((stage::plugin(files, config), error::plugin))
        .set_runner(|mut app| {
            loop {
                app.update();
                if let Some(exit) = app.should_exit() {
                    return exit;
                }
            }
        })
        .run();
}

#[cfg(test)]
mod test {
    include!(concat!(env!("OUT_DIR"), "/slang_tests.rs"));
    fn compile_and_run(path: &str, capture: &str) {
        let capture = std::fs::read(capture).ok();
        crate::compile(vec![path.to_string()], Default::default());
        let result = std::process::Command::new("/tmp/slexec").output().unwrap();
        println!("{}", String::try_from(result.stdout.clone()).unwrap());
        assert_eq!(result.status.code(), Some(0));
        if let Some(capture) = capture
            && capture != result.stdout
        {
            panic!("stdout differs");
        }
    }
}
