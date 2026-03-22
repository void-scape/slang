use crate::stage::Stage;
use bevy_app::prelude::*;
use bevy_ecs::error::Result;
use bevy_ecs::prelude::*;
use bevy_log::info;
use bevy_state::state::OnEnter;

pub fn plugin(files: Vec<String>) -> impl Fn(&mut App) {
    move |app| {
        let files = files.clone();
        let load_files = move |mut commands: Commands| -> Result {
            for file in files.iter() {
                let content = std::fs::read_to_string(file)?;
                info!(
                    "leaking source file {}b",
                    std::mem::size_of_val(content.as_bytes())
                );
                commands.spawn(StaticSourceFile(Box::leak(Box::new(SourceFile {
                    path: file.clone(),
                    content: content.leak(),
                }))));
            }
            Ok(())
        };
        app.add_systems(
            OnEnter(Stage::Tokenize),
            (load_files, tokenize_files, super::next_stage).chain(),
        );
    }
}

#[derive(Component)]
pub struct StaticSourceFile(&'static SourceFile);

#[derive(Debug, PartialEq, Eq)]
pub struct SourceFile {
    pub path: String,
    pub content: &'static str,
}

#[derive(Component)]
pub struct Tokens(pub Vec<Token>);

fn tokenize_files(
    mut commands: Commands,
    files: Query<(Entity, &StaticSourceFile), Without<Tokens>>,
) -> Result {
    for (entity, source) in files.iter() {
        let tokens = tokenize(source.0)?;
        info!(
            "allocated tokens {}b",
            std::mem::size_of_val(tokens.as_slice())
        );
        commands.entity(entity).insert(Tokens(tokens));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct Token {
    pub span: Span,
    pub kind: TokenKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub location: &'static SourceFile,
}

impl Default for Span {
    fn default() -> Self {
        Self {
            start: 0,
            end: 0,
            location: const {
                &SourceFile {
                    path: String::new(),
                    content: "",
                }
            },
        }
    }
}

impl Span {
    pub fn collapse(self, rhs: Self) -> Self {
        debug_assert_eq!(self.location, rhs.location);
        Self {
            start: self.start.min(rhs.start),
            end: self.end.max(rhs.end),
            location: self.location,
        }
    }
}

macro_rules! token_kind {
    {
        keywords = [
            $($keyword:ident => $kstr:literal,)*
        ]
        delimiters = [
            $($delimiter:ident => $dstr:literal,)*
        ]
    } => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum TokenKind {
            Ident(&'static str),
            Integer(u64),
            Str(&'static str),
            $($keyword,)*
            $($delimiter,)*
        }

        impl TokenKind {
            fn keywords() -> &'static [(Self, &'static str)] {
                &[
                    $((Self::$keyword, concat!($kstr, " ")),)*
                ]
            }
            fn delimiters() -> &'static [(Self, &'static str)] {
                &[
                    $((Self::$delimiter, $dstr),)*
                ]
            }
        }

        impl std::fmt::Display for TokenKind {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::Ident(ident) => f.write_str(ident),
                    Self::Integer(integer) => f.write_str(&format!("{integer}")),
                    Self::Str(str) => f.write_str(&format!("{str:?}")),
                    $(Self::$keyword => f.write_str($kstr),)*
                    $(Self::$delimiter => f.write_str($dstr),)*
                }
            }
        }
    };
}

token_kind! {
    keywords = [
        Let => "let",
        Return => "return",
        If => "if",
        While => "while",
        Fn => "fn",
        Const => "const",
        Extern => "extern",
    ]
    delimiters = [
        Semi => ";",
        Colon => ":",
        Comma => ",",
        OpenCurly => "{",
        CloseCurly => "}",
        OpenParen => "(",
        CloseParen => ")",
        Arrow => "->",
        AddAssign => "+=",
        SubAssign => "-=",
        MulAssign => "*=",
        DivAssign => "/=",
        ModAssign => "%=",
        BitAndAssign => "&=",
        BitOrAssign => "|=",
        XorAssign => "^=",
        ShlAssign => "<<=",
        ShrAssign => ">>=",
        Eq => "==",
        Equals => "=",
        Ne => "!=",
        Shr => ">>",
        Ge => ">=",
        Gt => ">",
        Shl => "<<",
        Le => "<=",
        Lt => "<",
        And => "&&",
        BitAnd => "&",
        Or => "||",
        BitOr => "|",
        Xor => "^",
        Not => "!",
        Mod => "%",
        Plus => "+",
        Minus => "-",
        Asterisk => "*",
        Slash => "/",
        Variadic => "...",
    ]
}

struct Tokenizer<'a> {
    location: &'static SourceFile,
    input: &'a mut &'static str,
    index: usize,
}

impl<'a> Tokenizer<'a> {
    fn new(location: &'static SourceFile, input: &'a mut &'static str) -> Self {
        Self {
            input,
            index: 0,
            location,
        }
    }

    fn eat_whitespace(&mut self) {
        let end = self
            .input
            .chars()
            .take_while(char::is_ascii_whitespace)
            .map(char::len_utf8)
            .sum();
        *self.input = &self.input[end..];
        self.index += end;
    }

    fn eat_comment(&mut self) -> bool {
        if self.input.starts_with("//") {
            let skip = self.input.find('\n').unwrap_or(self.input.len());
            *self.input = &self.input[skip..];
            self.index += skip;
            true
        } else {
            false
        }
    }

    fn eat_string(&mut self) -> Result<Option<Token>, Error> {
        if let Some(inner) = self.input.strip_prefix('"') {
            let end = inner.find('"').ok_or(Error {
                kind: ErrorKind::StrDelim,
                index: self.index + 1,
                location: self.location,
            })?;
            let result = Ok(Some(Token {
                span: Span {
                    start: self.index,
                    end: self.index + end + 2,
                    location: self.location,
                },
                kind: TokenKind::Str(&inner[..end]),
            }));
            *self.input = &self.input[end + 2..];
            self.index += end + 2;
            result
        } else {
            Ok(None)
        }
    }
}

fn tokenize(location: &'static SourceFile) -> Result<Vec<Token>, Error> {
    let mut tokens = Vec::new();
    let mut input = location.content;
    let mut tok = Tokenizer::new(location, &mut input);
    'outer: while {
        tok.eat_whitespace();
        !tok.input.is_empty()
    } {
        if tok.eat_comment() {
            continue;
        }
        if let Some(token) = tok.eat_string()? {
            tokens.push(token);
            continue;
        }
        for (token, prefix) in TokenKind::keywords()
            .iter()
            .chain(TokenKind::delimiters().iter())
        {
            if let Some(next) = tok.input.strip_prefix(prefix) {
                let len = prefix.len();
                tokens.push(Token {
                    span: Span {
                        start: tok.index,
                        end: tok.index + len,
                        location,
                    },
                    kind: *token,
                });
                *tok.input = next;
                tok.index += len;
                continue 'outer;
            }
        }
        let rem = tok
            .input
            .chars()
            .take_while(|c| {
                !c.is_whitespace()
                    && TokenKind::delimiters()
                        .iter()
                        .all(|(_, d)| d.as_bytes()[0] as char != *c)
            })
            .map(char::len_utf8)
            .sum();
        let str_rem = &tok.input[..rem];
        if str_rem.starts_with("0x")
            || str_rem.starts_with("0b")
            || str_rem.chars().all(|c| c.is_numeric())
        {
            let err = Error {
                kind: ErrorKind::Integer(str_rem),
                index: tok.index,
                location,
            };
            let parsed = if let Some(hex) = str_rem.strip_prefix("0x") {
                u64::from_str_radix(hex, 16).map_err(|_| err)?
            } else if let Some(bin) = str_rem.strip_prefix("0b") {
                u64::from_str_radix(bin, 2).map_err(|_| err)?
            } else {
                str_rem.parse().map_err(|_| err)?
            };

            tokens.push(Token {
                span: Span {
                    start: tok.index,
                    end: tok.index + rem,
                    location,
                },
                kind: TokenKind::Integer(parsed),
            });
            *tok.input = &tok.input[rem..];
            tok.index += rem;
        } else if str_rem
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
        {
            tokens.push(Token {
                span: Span {
                    start: tok.index,
                    end: tok.index + rem,
                    location,
                },
                kind: TokenKind::Ident(str_rem),
            });
            *tok.input = &tok.input[rem..];
            tok.index += rem;
        } else {
            return Err(Error {
                kind: ErrorKind::Ident(str_rem),
                index: tok.index,
                location,
            });
        }
    }
    Ok(tokens)
}

#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub index: usize,
    pub location: &'static SourceFile,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ErrorKind {
    StrDelim,
    Ident(&'static str),
    Integer(&'static str),
}

impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ErrorKind::StrDelim => f.write_str("Missing string delimiter"),
            ErrorKind::Ident(ident) => f.write_str(&format!("Invalid ident `{ident}`")),
            ErrorKind::Integer(integer) => f.write_str(&format!("Invalid integer `{integer}`")),
        }
    }
}
