use crate::{Flags, stage::Stage};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_state::state::OnEnter;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Stage::Post), (post, super::next_stage));
}

fn post(flags: Single<&Flags>) {
    let mut output = flags.output();
    output.insert_str(0, "./");
    if flags.capture {
        println!("Capturing {output}...");
        let result = std::process::Command::new(&output).output().unwrap();
        let path = "capture.txt";
        std::fs::write(path, result.stdout).unwrap();
        println!("Captured stdout to {}", path);
        println!("{}", result.status);
    } else if flags.run {
        println!("Running {output}...");
        let result = std::process::Command::new(&output).status().unwrap();
        println!("{}", result);
    }
}
