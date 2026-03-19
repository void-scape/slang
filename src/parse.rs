use crate::{
    ir::{Arg, BinOp, Data, Func, Ir, UnaryOp},
    tokenize::{Span, Token, TokenKind},
};
use std::{collections::HashMap, io::IsTerminal, panic::Location};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub span: Span,
    pub location: &'static Location<'static>,
}

impl Error {
    #[track_caller]
    fn recoverable() -> Self {
        Self {
            kind: ErrorKind::Recoverable,
            span: Span::default(),
            location: Location::caller(),
        }
    }
}

impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ErrorKind::Recoverable => f.write_str("Recoverable error not handled, this is a bug!"),
            ErrorKind::Expected { expected, got } => {
                if let Some(got) = got {
                    f.write_str(&format!("Expected `{expected}`, got `{got}`"))
                } else {
                    f.write_str(&format!("Expected `{expected}`"))
                }
            }
            ErrorKind::ExpectedIdent { got } => {
                f.write_str(&format!("Expected identifier, got `{got}`"))
            }
            ErrorKind::UnmatchedDelimiter { delimiter } => {
                f.write_str(&format!("Unmatched delimiter `{delimiter}`"))
            }
            ErrorKind::Expression { got } => f.write_str(&format!("Invalid expression `{got}`")),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Recoverable,
    Expected {
        expected: TokenKind,
        got: Option<TokenKind>,
    },
    ExpectedIdent {
        got: TokenKind,
    },
    UnmatchedDelimiter {
        delimiter: TokenKind,
    },
    Expression {
        got: TokenKind,
    },
}

pub fn pretty_print(path: &str, input: &str, err: Error) {
    let stdout = std::io::stdout();
    let mut position = 0;
    for (i, line) in input.lines().enumerate() {
        let len = line.chars().count() + 1;
        if len + position >= err.span.start {
            if stdout.is_terminal() {
                println!("\x1b[91m\x1b[4m{path}:{}: {err}\x1b[0m", i + 1);
            } else {
                println!("{path}:{}: {err}", i + 1);
            }
            #[cfg(debug_assertions)]
            println!("Generated at: {}", err.location);
            println!("> {line}");
            break;
        }
        position += len;
    }
}

// TODO: Memory opt: push and pop args off the stack as they come in and out of
// scope such that the total stack length may be reduced.
#[derive(Default)]
struct Parser {
    arg_map: HashMap<&'static str, Arg>,
    arg: usize,
    label: usize,
}

impl Parser {
    fn var(&mut self, ident: &'static str) -> Arg {
        *self.arg_map.entry(ident).or_insert_with(|| {
            let arg = self.arg;
            self.arg += 1;
            Arg::Var(arg)
        })
    }

    fn anonymous(&mut self) -> Arg {
        let var = self.arg;
        self.arg += 1;
        Arg::Var(var)
    }

    fn label(&mut self) -> usize {
        let label = self.label;
        self.label += 1;
        label
    }
}

impl Token {
    #[track_caller]
    fn is_kind(&self, kind: TokenKind) -> Result<()> {
        (self.kind == kind).ok_or(Error {
            span: self.span,
            kind: ErrorKind::Expected {
                expected: kind,
                got: Some(self.kind),
            },
            location: Location::caller(),
        })
    }

    fn is_kind_recoverable(&self, kind: TokenKind) -> Result<()> {
        (self.kind == kind).ok_or_else(Error::recoverable)
    }

    #[track_caller]
    fn ident(&self) -> Result<&'static str> {
        match self.kind {
            TokenKind::Ident(ident) => Ok(ident),
            token => Err(Error {
                span: self.span,
                kind: ErrorKind::ExpectedIdent { got: token },
                location: Location::caller(),
            }),
        }
    }
}

#[track_caller]
fn find_matching(tokens: &[Token], open: TokenKind, close: TokenKind) -> Result<usize> {
    let o = &mut 0;
    tokens
        .iter()
        .position(|t| {
            if t.kind == open {
                *o += 1;
            } else if t.kind == close {
                *o -= 1;
            }
            *o == 0
        })
        .ok_or_else(|| Error {
            span: tokens[0].span,
            kind: ErrorKind::UnmatchedDelimiter {
                delimiter: tokens[0].kind,
            },
            location: Location::caller(),
        })
}

fn args(parser: &mut Parser, ops: &mut Vec<Ir>, tokens: &[Token]) -> Result<Vec<Arg>> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    tokens
        .split(|t| t.kind == TokenKind::Comma)
        .map(|mut arg_tokens| bin_op(parser, ops, &mut arg_tokens, 0))
        .collect()
}

#[track_caller]
fn term(parser: &mut Parser, ops: &mut Vec<Ir>, tokens: &mut &[Token]) -> Result<Arg> {
    let arg = match tokens[0].kind {
        TokenKind::Integer(imm) => Arg::Lit(imm),
        TokenKind::Str(str) => Arg::Data(Data::Str(str)),
        TokenKind::Ident(ident) => {
            if tokens
                .get(1)
                .is_some_and(|t| t.kind == TokenKind::OpenParen)
            {
                *tokens = &tokens[1..];
                let end_args = find_matching(tokens, TokenKind::OpenParen, TokenKind::CloseParen)?;
                let args = args(parser, ops, &tokens[1..end_args])?;
                *tokens = &tokens[end_args + 1..];
                ops.push(Ir::Call {
                    symbol: ident,
                    args,
                });
                return Ok(Arg::CallReturn(ident));
            } else {
                parser.var(ident)
            }
        }
        TokenKind::Not => {
            *tokens = &tokens[1..];
            let arg = term(parser, ops, tokens)?;
            let dst = parser.anonymous();
            ops.push(Ir::Unary {
                dst,
                src: arg,
                una: UnaryOp::Not,
            });
            return Ok(dst);
        }
        TokenKind::OpenParen => {
            *tokens = &tokens[1..];
            let arg = bin_op(parser, ops, tokens, 0)?;
            tokens[0].is_kind(TokenKind::CloseParen)?;
            arg
        }
        term => {
            return Err(Error {
                span: tokens[0].span,
                kind: ErrorKind::Expression { got: term },
                location: Location::caller(),
            });
        }
    };
    *tokens = &tokens[1..];
    Ok(arg)
}

impl BinOp {
    // Precedence according to the rust standard, of which I am familiar with:
    // https://doc.rust-lang.org/reference/expressions.html
    const TABLE: &[&[BinOp]] = &[
        &[BinOp::Or],
        &[BinOp::And],
        &[
            BinOp::Gt,
            BinOp::Ge,
            BinOp::Lt,
            BinOp::Le,
            BinOp::Eq,
            BinOp::Ne,
        ],
        &[BinOp::BitOr],
        &[BinOp::Xor],
        &[BinOp::BitAnd],
        &[BinOp::Shr, BinOp::Shl],
        &[BinOp::Add, BinOp::Sub],
        &[BinOp::Mul, BinOp::Div, BinOp::Mod],
    ];
    fn max_precedence() -> usize {
        Self::TABLE.len()
    }
    fn precedence(&self) -> usize {
        Self::TABLE
            .iter()
            .enumerate()
            .find_map(|(i, s)| s.contains(self).then_some(i + 1))
            .unwrap()
    }
    fn from_token(token: TokenKind) -> Option<Self> {
        match token {
            TokenKind::Plus => Some(Self::Add),
            TokenKind::Minus => Some(Self::Sub),
            TokenKind::Asterisk => Some(Self::Mul),
            TokenKind::Slash => Some(Self::Div),
            TokenKind::Mod => Some(Self::Mod),
            //
            TokenKind::Eq => Some(Self::Eq),
            TokenKind::Ne => Some(Self::Ne),
            TokenKind::Gt => Some(Self::Gt),
            TokenKind::Ge => Some(Self::Ge),
            TokenKind::Lt => Some(Self::Lt),
            TokenKind::Le => Some(Self::Le),
            //
            TokenKind::And => Some(Self::And),
            TokenKind::Or => Some(Self::Or),
            //
            TokenKind::BitAnd => Some(Self::BitAnd),
            TokenKind::BitOr => Some(Self::BitOr),
            TokenKind::Xor => Some(Self::Xor),
            TokenKind::Shr => Some(Self::Shr),
            TokenKind::Shl => Some(Self::Shl),
            _ => None,
        }
    }
}

// Stolen from tsoding's B compiler:
// https://github.com/bext-lang/b/blob/main/src/b.rs#L515
fn bin_op(
    parser: &mut Parser,
    ops: &mut Vec<Ir>,
    tokens: &mut &[Token],
    precedence: usize,
) -> Result<Arg> {
    if precedence > BinOp::max_precedence() {
        return term(parser, ops, tokens);
    }

    let mut lhs = bin_op(parser, ops, tokens, precedence + 1)?;
    let mut saved = *tokens;

    if !tokens.is_empty()
        && let Some(op) = BinOp::from_token(tokens[0].kind)
        && op.precedence() == precedence
    {
        while !tokens.is_empty()
            && let Some(op) = BinOp::from_token(tokens[0].kind)
            && op.precedence() == precedence
        {
            *tokens = &tokens[1..];
            let dst = parser.anonymous();
            match op {
                BinOp::And => {
                    let skip = parser.label();
                    ops.push(Ir::Store { dst, src: lhs });
                    ops.push(Ir::JumpZero {
                        label: skip,
                        arg: lhs,
                    });
                    let rhs = bin_op(parser, ops, tokens, precedence + 1)?;
                    ops.push(Ir::Store { dst, src: rhs });
                    ops.push(Ir::Label { label: skip });
                }
                BinOp::Or => {
                    let skip = parser.label();
                    ops.push(Ir::Store { dst, src: lhs });
                    ops.push(Ir::JumpNotZero {
                        label: skip,
                        arg: lhs,
                    });
                    let rhs = bin_op(parser, ops, tokens, precedence + 1)?;
                    ops.push(Ir::Store { dst, src: rhs });
                    ops.push(Ir::Label { label: skip });
                }
                _ => {
                    let rhs = bin_op(parser, ops, tokens, precedence + 1)?;
                    ops.push(Ir::Bin {
                        dst,
                        lhs,
                        rhs,
                        bin: op,
                    });
                }
            }
            lhs = dst;
            saved = *tokens;
        }
    }

    *tokens = saved;
    Ok(lhs)
}

#[track_caller]
fn find_kind(tokens: &[Token], kind: TokenKind) -> Result<usize> {
    tokens
        .iter()
        .position(|t| t.kind == kind)
        .ok_or_else(|| Error {
            span: tokens[tokens.len() - 1].span,
            kind: ErrorKind::Expected {
                expected: kind,
                got: None,
            },
            location: Location::caller(),
        })
}

// TODO: Memory opt: there should be a way to skip the intermediate stack
// values by using registers directly, although this work will most likely
// have to be done in the asm phase or a second opt pass.
fn assign(parser: &mut Parser, ops: &mut Vec<Ir>, tokens: &mut &[Token]) -> Result<()> {
    if tokens[0].is_kind_recoverable(TokenKind::Let).is_ok() {
        let dst = parser.var(tokens[1].ident()?);
        tokens[2].is_kind(TokenKind::Equals)?;
        *tokens = &tokens[3..];
        let src = bin_op(parser, ops, tokens, 0)?;
        tokens[0].is_kind(TokenKind::Semi)?;
        *tokens = &tokens[1..];
        ops.push(Ir::Store { dst, src });
        Ok(())
    } else if tokens[1].is_kind_recoverable(TokenKind::Equals).is_ok() {
        let dst = parser.var(tokens[0].ident()?);
        tokens[1].is_kind(TokenKind::Equals)?;
        *tokens = &tokens[2..];
        let src = bin_op(parser, ops, tokens, 0)?;
        tokens[0].is_kind(TokenKind::Semi)?;
        *tokens = &tokens[1..];
        ops.push(Ir::Store { dst, src });
        Ok(())
    } else {
        Err(Error::recoverable())
    }
}

fn ret(parser: &mut Parser, ops: &mut Vec<Ir>, tokens: &mut &[Token]) -> Result<()> {
    tokens[0].is_kind_recoverable(TokenKind::Return)?;
    let end = find_kind(tokens, TokenKind::Semi)?;
    if end == 1 {
        ops.push(Ir::Return { arg: None });
        *tokens = &tokens[2..];
    } else {
        *tokens = &tokens[1..];
        let arg = bin_op(parser, ops, tokens, 0)?;
        ops.push(Ir::Return { arg: Some(arg) });
        tokens[0].is_kind(TokenKind::Semi)?;
        *tokens = &tokens[1..];
    }
    Ok(())
}

fn iff(parser: &mut Parser, ops: &mut Vec<Ir>, tokens: &mut &[Token]) -> Result<()> {
    tokens[0].is_kind_recoverable(TokenKind::If)?;
    let end_condition = find_kind(tokens, TokenKind::OpenCurly)?;
    let condition = bin_op(parser, ops, &mut &tokens[1..end_condition], 0)?;
    *tokens = &tokens[end_condition..];
    let body = block(parser, tokens)?;
    let zero_label = parser.label();
    ops.push(Ir::JumpZero {
        label: zero_label,
        arg: condition,
    });
    ops.extend(body);
    ops.push(Ir::Label { label: zero_label });
    Ok(())
}

fn whil(parser: &mut Parser, ops: &mut Vec<Ir>, tokens: &mut &[Token]) -> Result<()> {
    tokens[0].is_kind_recoverable(TokenKind::While)?;
    let evaluate = parser.label();
    ops.push(Ir::Label { label: evaluate });
    *tokens = &tokens[1..];
    let condition = bin_op(parser, ops, tokens, 0)?;
    let body = block(parser, tokens)?;
    let exit_label = parser.label();
    ops.push(Ir::JumpZero {
        label: exit_label,
        arg: condition,
    });
    ops.extend(body);
    ops.push(Ir::Jump { label: evaluate });
    ops.push(Ir::Label { label: exit_label });
    Ok(())
}

fn lvalue(parser: &mut Parser, ops: &mut Vec<Ir>, tokens: &mut &[Token]) -> Result<()> {
    let _lvalue = bin_op(parser, ops, tokens, 0)?;
    tokens[0].is_kind(TokenKind::Semi)?;
    *tokens = &tokens[1..];
    Ok(())
}

fn block(parser: &mut Parser, tokens: &mut &[Token]) -> Result<Vec<Ir>> {
    let mut ops = Vec::new();
    // TODO: This is always non recoverable?
    tokens[0].is_kind(TokenKind::OpenCurly)?;
    let end = find_matching(tokens, TokenKind::OpenCurly, TokenKind::CloseCurly)?;
    let block = &mut &tokens[1..end];
    *tokens = &tokens[end + 1..];
    fn recoverable(result: Result<()>) -> Result<bool> {
        match result {
            Ok(_) => Ok(true),
            Err(err) => {
                if err.kind == ErrorKind::Recoverable {
                    Ok(false)
                } else {
                    Err(err)
                }
            }
        }
    }
    'outer: while !block.is_empty() {
        for f in [ret, assign, iff, whil, lvalue] {
            if recoverable(f(parser, &mut ops, block))? {
                continue 'outer;
            }
        }
    }
    Ok(ops)
}

#[track_caller]
fn params(parser: &mut Parser, tokens: &[Token]) -> Result<(bool, Vec<Arg>)> {
    if tokens.is_empty() {
        return Ok((false, Vec::new()));
    }
    let mut variadic = false;
    let args = tokens
        .split(|t| t.kind == TokenKind::Comma)
        .filter_map(|param| {
            assert_eq!(param.len(), 1);
            match param[0].kind {
                TokenKind::Variadic => {
                    variadic = true;
                    None
                }
                TokenKind::Ident(ident) => Some(Ok(parser.var(ident))),
                got => Some(Err(Error {
                    span: param[0].span,
                    kind: ErrorKind::Expression { got },
                    location: Location::caller(),
                })),
            }
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((variadic, args))
}

fn func(parser: &mut Parser, tokens: &mut &[Token]) -> Result<Func> {
    // TODO: This is always non recoverable?
    tokens[0].is_kind(TokenKind::Fn)?;
    let ident = tokens[1].ident()?;
    tokens[2].is_kind(TokenKind::OpenParen)?;
    *tokens = &tokens[2..];
    let end_args = find_matching(tokens, TokenKind::OpenParen, TokenKind::CloseParen)?;
    let (variadic, params) = params(parser, &tokens[1..end_args])?;
    *tokens = &tokens[end_args + 1..];
    let mut returns = Vec::new();
    if tokens[0].is_kind_recoverable(TokenKind::Arrow).is_ok() {
        tokens[1].is_kind(TokenKind::Ident("u64"))?;
        returns.push(());
        *tokens = &tokens[2..];
    }
    let body = block(parser, tokens)?;
    Ok(Func {
        ident,
        params,
        variadic,
        returns,
        body,
    })
}

pub fn parse(tokens: &mut &[Token]) -> Result<Vec<Func>> {
    let mut funcs = Vec::new();
    let mut parser = Parser::default();
    while !tokens.is_empty() {
        funcs.push(func(&mut parser, tokens)?);
        parser.arg_map.clear();
        parser.arg = 0;
    }
    Ok(funcs)
}
