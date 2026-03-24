use crate::Flags;
use crate::error::Report;
use crate::stage::Stage;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_log::info;
use bevy_state::state::OnEnter;

type Result<T> = std::result::Result<T, crate::error::Error>;

pub fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(Stage::Tokenize),
        (load_files, tokenize_files, super::next_stage).chain(),
    );
}

#[derive(Component)]
pub struct StaticSourceFile(&'static SourceFile);

fn load_files(mut commands: Commands, flags: Single<&Flags>) -> bevy_ecs::error::Result {
    for input in flags.input.iter() {
        let content = std::fs::read_to_string(input)?;
        info!(
            "leaking source file {}b",
            std::mem::size_of_val(content.as_bytes())
        );
        commands.spawn(StaticSourceFile(Box::leak(Box::new(SourceFile {
            path: input.clone(),
            content: content.leak(),
        }))));
    }
    Ok(())
}

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
) -> bevy_ecs::error::Result {
    fn tokenize(location: &'static SourceFile) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        let mut input = location.content;
        if input.is_empty() {
            return Ok(Vec::new());
        }
        let mut tok = Tokenizer::new(location, &mut input);

        while {
            tok.eat_whitespace();
            !tok.input.is_empty()
        } {
            if tok.eat_comment() {
                continue;
            }
            for parser in [
                Tokenizer::eat_string,
                Tokenizer::eat_keyword_or_delimiter,
                Tokenizer::eat_integer,
                Tokenizer::eat_ident,
            ] {
                if let Some(token) = parser(&mut tok)? {
                    tokens.push(token);
                    break;
                }
            }
        }
        Ok(tokens)
    }

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

macro_rules! token_kind {
    {
        keywords = [$($keyword:ident => $kstr:literal,)*]
        delimiters = [$($delimiter:ident => $dstr:literal,)*]
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
                &[$((Self::$keyword, concat!($kstr, " ")),)*]
            }
            fn delimiters() -> &'static [(Self, &'static str)] {
                &[$((Self::$delimiter, $dstr),)*]
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
        Bang => "!",
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

    fn span(&self, start: usize, end: usize) -> Span {
        Span {
            start,
            end,
            location: self.location,
        }
    }

    fn eat(&mut self) {
        if !self.input.is_empty() {
            *self.input = &self.input[1..];
            self.index += 1;
        }
    }

    fn eatn(&mut self, n: usize) {
        for _ in 0..n {
            self.eat();
        }
    }

    fn eat_while(&mut self, c: impl Fn(&u8) -> bool) {
        while !self.input.is_empty() && c(&self.input.as_bytes()[0]) {
            self.eat();
        }
    }

    fn eat_whitespace(&mut self) {
        self.eat_while(u8::is_ascii_whitespace);
    }

    fn starts_with(&self, bytes: &[u8]) -> bool {
        self.input.len() >= bytes.len() && &self.input.as_bytes()[..bytes.len()] == bytes
    }

    fn eat_comment(&mut self) -> bool {
        let is_comment = self.starts_with(b"//");
        if is_comment {
            self.eat_while(|b| *b != b'\n');
        }
        is_comment
    }

    fn eat_string(&mut self) -> Result<Option<Token>> {
        if self.starts_with(b"\"") {
            let chk = *self.input;
            let start = self.index;
            self.eat();
            let str_start = self.index;
            self.eat_while(|b| *b != b'\"');
            if !self.starts_with(b"\"") {
                let span = self.span(start, start + 1);
                return Err(span.custom("Missing matching `\"`"));
            }
            let str_end = self.index;
            self.eat();
            let end = self.index;
            Ok(Some(Token {
                span: self.span(start, end),
                kind: TokenKind::Str(&chk[str_start - start..str_end - start]),
            }))
        } else {
            Ok(None)
        }
    }

    fn eat_keyword_or_delimiter(&mut self) -> Result<Option<Token>> {
        for (kind, prefix) in TokenKind::keywords()
            .iter()
            .chain(TokenKind::delimiters().iter())
        {
            if self.starts_with(prefix.as_bytes()) {
                let start = self.index;
                self.eatn(prefix.trim_end().len());
                let end = self.index;
                return Ok(Some(Token {
                    span: self.span(start, end),
                    kind: *kind,
                }));
            }
        }
        Ok(None)
    }

    fn eat_integer_radix(
        &mut self,
        prefix: &[u8],
        radix: u32,
        condition: impl Fn(&u8) -> bool,
    ) -> Result<Option<Token>> {
        let chk = *self.input;
        let start = self.index;
        self.eatn(prefix.len());
        self.eat_while(condition);
        let end = self.index;
        let span = self.span(start, end);
        Ok(Some(Token {
            span,
            kind: TokenKind::Integer(
                u64::from_str_radix(&chk[prefix.len()..end - start], radix)
                    .map_err(|_| span.custom("Invalid integer"))?,
            ),
        }))
    }

    fn eat_integer(&mut self) -> Result<Option<Token>> {
        if self.starts_with(b"0x") {
            self.eat_integer_radix(b"0x", 16, u8::is_ascii_hexdigit)
        } else if self.starts_with(b"0b") {
            self.eat_integer_radix(b"0b", 2, |b| *b == b'0' || *b == b'1')
        } else if self.input.starts_with(|c: char| c.is_ascii_digit()) {
            self.eat_integer_radix(b"", 10, u8::is_ascii_digit)
        } else {
            Ok(None)
        }
    }

    fn eat_ident(&mut self) -> Result<Option<Token>> {
        if self
            .input
            .starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        {
            let chk = *self.input;
            let start = self.index;
            self.eat_while(|b| b.is_ascii_alphanumeric() || *b == b'_');
            let end = self.index;
            Ok(Some(Token {
                span: self.span(start, end),
                kind: TokenKind::Ident(&chk[..end - start]),
            }))
        } else {
            Err(self
                .span(self.index, self.index + 1)
                .custom("Invalid input"))
        }
    }
}
