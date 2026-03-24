use crate::stage::{Stage, tokenize::Span};
use bevy_app::prelude::*;
use bevy_ecs::{
    error::{BevyError, ErrorContext},
    prelude::*,
};
use bevy_log::error;
use bevy_state::state::State;
use std::{
    io::IsTerminal,
    panic::Location,
    sync::atomic::{AtomicUsize, Ordering},
};

pub fn plugin(app: &mut App) {
    app.set_error_handler(pretty_print);
}

thread_local! { static ERROR: AtomicUsize = const { AtomicUsize::new(0) }; }
pub fn check_stage_error(stage: Res<State<Stage>>, mut writer: MessageWriter<AppExit>) {
    ERROR.with(|err| {
        if err.load(Ordering::SeqCst) > 0 {
            error!("Error encountered in {:?}", *stage);
            writer.write(AppExit::error());
        }
    });
}

#[inline]
#[track_caller]
fn pretty_print(error: BevyError, ctx: ErrorContext) {
    ERROR.with(|err| {
        err.fetch_add(1, Ordering::SeqCst);
    });
    if let Some(err) = error.downcast_ref::<Error>() {
        pretty_print_error(err, &ctx.name());
    } else {
        print!("{error}");
        bevy_ecs::error::error(error, ctx);
    }
}

fn pretty_print_error(err: &Error, _bevy_location: &impl std::fmt::Display) {
    let path = &err.span.location.path;
    let content = err.span.location.content;
    let stdout = std::io::stdout();
    let start = err.span.start;
    let end = err.span.end;
    let _file_location = err.location;

    let line = content[..start].chars().filter(|c| *c == '\n').count() + 1;
    let column = content[..start]
        .lines()
        .next_back()
        .map(|l| l.len())
        .unwrap_or_default()
        + 1;
    if stdout.is_terminal() {
        println!("\x1b[91m\x1b[4m{path}:{}:{}: {err}\x1b[0m", line, column);
    } else {
        println!("{path}:{}:{}: {err}", line, column);
    }
    #[cfg(debug_assertions)]
    if stdout.is_terminal() {
        println!(
            "\x1b[38;5;243m\x1b[4m{}\x1b[24m: {}\x1b[0m",
            _file_location, _bevy_location,
        );
    } else {
        println!("{}: {}", _file_location, _bevy_location);
    }

    fn cleaned_print(str: &str) -> usize {
        let str = str.replace("\r", "").replace("\t", "    ");
        print!("{}", str);
        str.chars().count()
    }

    let mut prev_lines = content[..start].lines().rev();
    let (cur, prev) = (prev_lines.next(), prev_lines.next());
    if let Some(prev) = prev {
        print!("  ");
        cleaned_print(prev);
        println!();
    }
    let mut pad = 2;
    print!("> ");
    if let Some(cur) = cur {
        pad += cleaned_print(cur);
    }

    if let Some(cur) = content[start..].lines().next() {
        cleaned_print(cur);
        println!();
    }
    for _ in 0..pad {
        print!(" ");
    }
    for _ in start..end {
        print!("^");
    }
    println!();
}

#[derive(Debug)]
pub struct Error {
    pub span: Span,
    pub kind: ErrorKind,
    pub location: &'static Location<'static>,
}

#[derive(Debug)]
pub enum ErrorKind {
    Custom(&'static str),
    Msg(String),
}

impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ErrorKind::Custom(msg) => f.write_str(msg),
            ErrorKind::Msg(msg) => f.write_str(msg),
        }
    }
}

pub trait SpannedReport {
    fn spanned(&self, kind: ErrorKind) -> Error;
}

impl SpannedReport for Span {
    #[track_caller]
    fn spanned(&self, kind: ErrorKind) -> Error {
        Error {
            span: *self,
            kind,
            location: Location::caller(),
        }
    }
}

impl<T> Report for T where T: SpannedReport {}
pub trait Report: SpannedReport {
    #[track_caller]
    fn custom(&self, msg: &'static str) -> Error {
        self.spanned(ErrorKind::Custom(msg))
    }
    #[track_caller]
    fn msg(&self, msg: String) -> Error {
        self.spanned(ErrorKind::Msg(msg))
    }
}
