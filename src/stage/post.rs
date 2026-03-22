use crate::stage::Stage;
use bevy_app::prelude::*;
use bevy_ecs::message::MessageWriter;
use bevy_state::state::OnEnter;

#[derive(Default)]
pub struct PostJobPlugin {
    pub run: bool,
    pub capture: bool,
}

impl Plugin for PostJobPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Stage::Post), post(self.run, self.capture));
    }
}

fn post(run: bool, capture: bool) -> impl FnMut(MessageWriter<AppExit>) {
    move |mut writer| {
        println!("Running /tmp/slexec");
        if capture {
            let result = std::process::Command::new("/tmp/slexec").output().unwrap();
            let path = "capture.txt";
            std::fs::write(path, result.stdout).unwrap();
            println!("Captured stdout to {}", path);
            println!("{}", result.status);
        } else if run {
            let result = std::process::Command::new("/tmp/slexec").status().unwrap();
            println!("{}", result);
        }
        writer.write(AppExit::Success);
    }
}
