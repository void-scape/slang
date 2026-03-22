use crate::{Config, stage::post::PostJobPlugin};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_state::{
    app::{AppExtStates, StatesPlugin},
    commands::CommandsStatesExt,
    state::{State, States},
};

pub mod codegen;
pub mod ir;
pub mod parse;
pub mod post;
pub mod tokenize;
pub mod ty;
pub mod variable;

/// # Architecture
/// [`SourceFile`]s are spawned in the world and serve as the root nodes of
/// the compiler. [`Stage::Tokenize`] inserts [`Tokens`] into the [`SourceFile`]s.
/// [`Stage::Parse`] interprets those [`Tokens`] and spawns the [`tree`] hierarchy
/// as children of [`SourceFile`]s. [`tree`] can be thought of as a traditional
/// AST.
///
/// [`Stage::Variable`], [`Stage::Type`], [`Stage::Ir`], and [`Stage::Codegen`]
/// all operate on the [`tree`] hierarchy directly.
///
/// If a [`Stage`] encounters an unrecoverable error, the error is reported
/// immediately and upon exiting the [`Stage`], the program will exit.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, States)]
pub enum Stage {
    #[default]
    Tokenize,
    Parse,
    Variable,
    Type,
    Ir,
    Codegen,
    Post,
}

pub fn next_stage(mut commands: Commands, stage: Res<State<Stage>>) {
    let stages = [
        Stage::Tokenize,
        Stage::Parse,
        Stage::Variable,
        Stage::Type,
        Stage::Ir,
        Stage::Codegen,
        Stage::Post,
    ];
    let value = *stage.get() as usize;
    if let Some(state) = stages.get(value + 1) {
        println!("current {:?}", stage.get());
        println!("set {state:?} {}", value + 1);
        commands.set_state(*state);
    }
}

pub fn plugin(files: Vec<String>, config: Config) -> impl Fn(&mut App) {
    move |app| {
        app.add_plugins(StatesPlugin)
            .add_plugins((
                tokenize::plugin(files.clone()),
                parse::plugin,
                variable::plugin,
                ty::plugin,
                ir::plugin,
                codegen::plugin,
                PostJobPlugin {
                    run: config.run,
                    capture: config.capture,
                },
            ))
            .init_state::<Stage>();
    }
}
