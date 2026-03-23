use crate::Timing;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_log::info;
use bevy_state::{
    app::{AppExtStates, StatesPlugin},
    commands::CommandsStatesExt,
    state::{State, States},
};

pub mod codegen;
pub mod fold;
pub mod ir;
pub mod parse;
pub mod post;
pub mod scope;
pub mod tokenize;
pub mod ty;

/// # Architecture
/// [`SourceFile`]s are spawned in the world and serve as the root nodes of
/// the compiler. [`Stage::Tokenize`] inserts [`Tokens`] into the [`SourceFile`]s.
/// [`Stage::Parse`] interprets those [`Tokens`] and spawns the [`tree`] hierarchy
/// as children of [`SourceFile`]s. [`tree`] can be thought of as a traditional
/// AST.
///
/// [`Stage::Scope`], [`Stage::Type`], [`Stage::Ir`], and [`Stage::Codegen`]
/// all operate on the [`tree`] hierarchy directly.
///
/// If a [`Stage`] encounters an unrecoverable error, the error is reported
/// immediately and upon exiting the [`Stage`], the program will exit.
macro_rules! stage {
    {
        $($variant:ident,)*
    } => {
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, States)]
        pub enum Stage {
            #[default]
            $($variant,)*
        }
        impl Stage {
            const STAGES: &[Self] = &[$(Self::$variant,)*];
        }
    };
}

stage! {
    Tokenize,
    Parse,
    Scope,
    Type,
    Fold,
    Ir,
    Codegen,
    Post,
}

pub fn next_stage(
    mut commands: Commands,
    stage: Res<State<Stage>>,
    mut writer: MessageWriter<AppExit>,
    mut timing: Single<&mut Timing>,
) {
    let stages = Stage::STAGES;
    let value = *stage.get() as usize;
    if let Some(state) = stages.get(value + 1) {
        commands.run_system_cached(crate::error::check_stage_error);
        timing.finished(*stage.get());
        info!("Finished {:?}, entering {:?}", stage.get(), state);
        commands.set_state(*state);
    } else {
        timing.report();
        writer.write(AppExit::Success);
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins(StatesPlugin)
        .add_plugins((
            tokenize::plugin,
            parse::plugin,
            scope::plugin,
            fold::plugin,
            ty::plugin,
            ir::plugin,
            codegen::plugin,
            post::plugin,
        ))
        .init_state::<Stage>();
}
