use crate::{
    arena::Arena,
    ast::{
        Ast, BinOp, BinOpKind, Block, Call, Declaration, Expr, Func, Ident, If, Literal,
        LiteralKind, Return, Spanned, Type, TypeKind, UnaryOp, UnaryOpKind, While,
    },
    tokenize::{Span, Token, TokenKind},
};
use std::{io::IsTerminal, panic::Location};

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
            ErrorKind::Declaraction { kind } => f.write_str(&format!("Invalid {kind} declaration")),
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
    Declaraction {
        kind: &'static str,
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
        .ok_or(Error {
            span: tokens[0].span,
            kind: ErrorKind::UnmatchedDelimiter {
                delimiter: tokens[0].kind,
            },
            location: Location::caller(),
        })
}

fn args(arena: &mut Arena, tokens: &[Token]) -> Result<Vec<Expr>> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    tokens
        .split(|t| t.kind == TokenKind::Comma)
        .map(|mut arg_tokens| bin_op(arena, &mut arg_tokens, 0))
        .collect()
}

fn expr(arena: &mut Arena, tokens: &mut &[Token]) -> Result<Expr> {
    let token = tokens[0];
    match token.kind {
        TokenKind::Let => {
            *tokens = &tokens[1..];
            let ident = Ident {
                span: tokens[0].span,
                value: tokens[0].ident()?,
            };
            tokens[1].is_kind(TokenKind::Equals)?;
            *tokens = &tokens[2..];
            let rhs = bin_op(arena, tokens, 0)?;
            Ok(Expr::Declaration(Declaration {
                span: token.span.collapse(rhs.span()),
                ident,
                ty: Some(Type::new(TypeKind::U64)),
                rhs: Some(arena.allocate(rhs)),
            }))
        }
        TokenKind::Ident(ident) => {
            *tokens = &tokens[1..];
            if tokens
                .first()
                .is_some_and(|t| t.kind == TokenKind::OpenParen)
            {
                let end_args = find_matching(tokens, TokenKind::OpenParen, TokenKind::CloseParen)?;
                let arguments = args(arena, &tokens[1..end_args])?;
                let end_span = tokens[end_args].span;
                *tokens = &tokens[end_args + 1..];
                Ok(Expr::Call(Call {
                    span: token.span.collapse(end_span),
                    ident: Ident {
                        span: token.span,
                        value: ident,
                    },
                    arguments: arena.allocate_slice(&arguments),
                }))
            } else {
                Ok(Expr::Ident(Ident {
                    span: token.span,
                    value: ident,
                }))
            }
        }
        TokenKind::Integer(integer) => {
            *tokens = &tokens[1..];
            Ok(Expr::Literal(Literal {
                span: token.span,
                kind: LiteralKind::Integer(integer),
            }))
        }
        TokenKind::Str(str) => {
            *tokens = &tokens[1..];
            Ok(Expr::Literal(Literal {
                span: token.span,
                kind: LiteralKind::Str(str),
            }))
        }
        TokenKind::Not => {
            *tokens = &tokens[1..];
            let expr = expr(arena, tokens)?;
            Ok(Expr::UnaryOp(UnaryOp {
                span: token.span.collapse(expr.span()),
                kind: UnaryOpKind::Not,
                expr: arena.allocate(expr),
            }))
        }
        TokenKind::OpenParen => {
            *tokens = &tokens[1..];
            let expr = bin_op(arena, tokens, 0)?;
            tokens[0].is_kind(TokenKind::CloseParen)?;
            *tokens = &tokens[1..];
            Ok(expr)
        }
        got => Err(Error {
            span: tokens[0].span,
            kind: ErrorKind::Expression { got },
            location: Location::caller(),
        }),
    }
}

impl BinOpKind {
    // Precedence according to the rust standard, of which I am familiar with:
    // https://doc.rust-lang.org/reference/expressions.html
    const TABLE: &[&[BinOpKind]] = &[
        &[
            BinOpKind::Assign,
            BinOpKind::AddAssign,
            BinOpKind::SubAssign,
            BinOpKind::MulAssign,
            BinOpKind::DivAssign,
            BinOpKind::ModAssign,
            BinOpKind::BitAndAssign,
            BinOpKind::BitOrAssign,
            BinOpKind::XorAssign,
            BinOpKind::ShlAssign,
            BinOpKind::ShrAssign,
        ],
        &[BinOpKind::Or],
        &[BinOpKind::And],
        &[
            BinOpKind::Gt,
            BinOpKind::Ge,
            BinOpKind::Lt,
            BinOpKind::Le,
            BinOpKind::Eq,
            BinOpKind::Ne,
        ],
        &[BinOpKind::BitOr],
        &[BinOpKind::Xor],
        &[BinOpKind::BitAnd],
        &[BinOpKind::Shr, BinOpKind::Shl],
        &[BinOpKind::Add, BinOpKind::Sub],
        &[BinOpKind::Mul, BinOpKind::Div, BinOpKind::Mod],
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
            //
            TokenKind::Equals => Some(Self::Assign),
            TokenKind::AddAssign => Some(Self::AddAssign),
            TokenKind::SubAssign => Some(Self::SubAssign),
            TokenKind::MulAssign => Some(Self::MulAssign),
            TokenKind::DivAssign => Some(Self::DivAssign),
            TokenKind::ModAssign => Some(Self::ModAssign),
            TokenKind::BitAndAssign => Some(Self::BitAndAssign),
            TokenKind::BitOrAssign => Some(Self::BitOrAssign),
            TokenKind::XorAssign => Some(Self::XorAssign),
            TokenKind::ShlAssign => Some(Self::ShlAssign),
            TokenKind::ShrAssign => Some(Self::ShrAssign),
            _ => None,
        }
    }
}

// Stolen from tsoding's B compiler:
// https://github.com/bext-lang/b/blob/main/src/b.rs#L515
fn bin_op(arena: &mut Arena, tokens: &mut &[Token], precedence: usize) -> Result<Expr> {
    if precedence > BinOpKind::max_precedence() {
        return expr(arena, tokens);
    }

    let mut lhs = bin_op(arena, tokens, precedence + 1)?;
    let mut saved = *tokens;

    if !tokens.is_empty()
        && let Some(op) = BinOpKind::from_token(tokens[0].kind)
        && op.precedence() == precedence
    {
        while !tokens.is_empty()
            && let Some(op) = BinOpKind::from_token(tokens[0].kind)
            && op.precedence() == precedence
        {
            *tokens = &tokens[1..];
            let rhs = bin_op(arena, tokens, precedence + 1)?;
            lhs = Expr::BinOp(BinOp {
                span: lhs.span().collapse(rhs.span()),
                kind: op,
                lhs: arena.allocate(lhs),
                rhs: arena.allocate(rhs),
            });
            saved = *tokens;
        }
    }

    *tokens = saved;
    Ok(lhs)
}

fn ret(arena: &mut Arena, tokens: &mut &[Token]) -> Result<Ast> {
    let token = tokens[0];
    token.is_kind_recoverable(TokenKind::Return)?;
    *tokens = &tokens[1..];
    let ast = if tokens[0].kind == TokenKind::Semi {
        Ast::Return(Return {
            span: token.span,
            expr: None,
        })
    } else {
        let expr = bin_op(arena, tokens, 0)?;
        Ast::Return(Return {
            span: token.span.collapse(expr.span()),
            expr: Some(expr),
        })
    };
    tokens[0].is_kind(TokenKind::Semi)?;
    *tokens = &tokens[1..];
    Ok(ast)
}

fn block(arena: &mut Arena, tokens: &mut &[Token]) -> Result<Block> {
    let mut statements = Vec::new();
    let start = tokens[0];
    // TODO: This is always non recoverable?
    start.is_kind(TokenKind::OpenCurly)?;
    let end = find_matching(tokens, TokenKind::OpenCurly, TokenKind::CloseCurly)?;
    let block = &mut &tokens[1..end];
    let end_span = tokens[end].span;
    *tokens = &tokens[end + 1..];
    while !block.is_empty() {
        statements.push(ast(arena, block)?);
    }
    Ok(Block {
        span: start.span.collapse(end_span),
        statements: arena.allocate_slice(&statements),
    })
}

fn block_ast(arena: &mut Arena, tokens: &mut &[Token]) -> Result<Ast> {
    let mut statements = Vec::new();
    let start = tokens[0];
    start.is_kind_recoverable(TokenKind::OpenCurly)?;
    let end = find_matching(tokens, TokenKind::OpenCurly, TokenKind::CloseCurly)?;
    let block = &mut &tokens[1..end];
    let end_span = tokens[end].span;
    *tokens = &tokens[end + 1..];
    while !block.is_empty() {
        statements.push(ast(arena, block)?);
    }
    Ok(Ast::Block(Block {
        span: start.span.collapse(end_span),
        statements: arena.allocate_slice(&statements),
    }))
}

fn iff(arena: &mut Arena, tokens: &mut &[Token]) -> Result<Ast> {
    let token = tokens[0];
    token.is_kind_recoverable(TokenKind::If)?;
    *tokens = &tokens[1..];
    let condition = bin_op(arena, tokens, 0)?;
    let body = block(arena, tokens)?;
    Ok(Ast::If(If {
        span: token.span.collapse(body.span),
        condition,
        body,
    }))
}

fn whilee(arena: &mut Arena, tokens: &mut &[Token]) -> Result<Ast> {
    let token = tokens[0];
    token.is_kind_recoverable(TokenKind::While)?;
    *tokens = &tokens[1..];
    let condition = bin_op(arena, tokens, 0)?;
    let body = block(arena, tokens)?;
    Ok(Ast::While(While {
        span: token.span.collapse(body.span),
        condition,
        body,
    }))
}

fn lvalue(arena: &mut Arena, tokens: &mut &[Token]) -> Result<Ast> {
    let expr = bin_op(arena, tokens, 0)?;
    tokens[0].is_kind(TokenKind::Semi)?;
    *tokens = &tokens[1..];
    Ok(Ast::Expr(expr))
}

#[track_caller]
fn params(_arena: &mut Arena, tokens: &[Token]) -> Result<(bool, Vec<Declaration>)> {
    if tokens.is_empty() {
        return Ok((false, Vec::new()));
    }
    let start_span = tokens[0].span;
    let mut variadic = false;
    let args = tokens
        .split(|t| t.kind == TokenKind::Comma)
        .filter_map(|param| {
            if param.len() == 1 && param[0].kind == TokenKind::Variadic {
                variadic = true;
                return None;
            }
            if param.len() != 3 {
                return Some(Err(Error {
                    span: start_span,
                    kind: ErrorKind::Declaraction { kind: "parameter" },
                    location: Location::caller(),
                }));
            }
            let ident = match param[0].ident() {
                Ok(ident) => ident,
                Err(err) => return Some(Err(err)),
            };
            if let Err(err) = param[1].is_kind(TokenKind::Colon) {
                return Some(Err(err));
            }
            let ty = match param[2].ident() {
                Ok(ident) => ident,
                Err(err) => return Some(Err(err)),
            };
            assert_eq!(ty, "u64");
            Some(Ok(Declaration {
                span: param[0].span.collapse(param[2].span),
                ident: Ident {
                    span: param[0].span,
                    value: ident,
                },
                ty: Some(Type::new(TypeKind::U64)),
                rhs: None,
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((variadic, args))
}

fn func(arena: &mut Arena, tokens: &mut &[Token]) -> Result<Ast> {
    let token = tokens[0];
    token.is_kind_recoverable(TokenKind::Fn)?;
    let ident = tokens[1];
    tokens[2].is_kind(TokenKind::OpenParen)?;
    *tokens = &tokens[2..];
    let end_args = find_matching(tokens, TokenKind::OpenParen, TokenKind::CloseParen)?;
    let (variadic, arguments) = params(arena, &tokens[1..end_args])?;
    *tokens = &tokens[end_args + 1..];
    let mut returns = Vec::new();
    if tokens[0].is_kind_recoverable(TokenKind::Arrow).is_ok() {
        tokens[1].is_kind(TokenKind::Ident("u64"))?;
        returns.push(Declaration {
            span: tokens[1].span,
            ident: Ident::default(),
            ty: Some(Type::new(TypeKind::U64)),
            rhs: None,
        });
        *tokens = &tokens[2..];
    }
    let body = block(arena, tokens)?;
    Ok(Ast::Func(Func {
        span: token.span.collapse(body.span),
        ident: Ident {
            span: ident.span,
            value: ident.ident()?,
        },
        arguments: arena.allocate_slice(&arguments),
        returns: arena.allocate_slice(&returns),
        body,
        variadic,
    }))
}

#[track_caller]
fn ast(arena: &mut Arena, tokens: &mut &[Token]) -> Result<Ast> {
    fn recoverable<T>(result: Result<T>) -> Result<Option<T>> {
        match result {
            Ok(val) => Ok(Some(val)),
            Err(err) => {
                if err.kind == ErrorKind::Recoverable {
                    Ok(None)
                } else {
                    Err(err)
                }
            }
        }
    }
    for f in [func, ret, iff, whilee, lvalue, block_ast] {
        if let Some(ast) = recoverable(f(arena, tokens))? {
            return Ok(ast);
        }
    }
    // TODO: Change this error... what do we expect here?
    Err(Error {
        kind: ErrorKind::Expression {
            got: tokens[0].kind,
        },
        span: tokens[0].span,
        location: Location::caller(),
    })
}

pub fn parse(tokens: &mut &[Token]) -> Result<Vec<Ast>> {
    let mut tree = Vec::new();
    let mut arena = Arena::new(1024);
    while !tokens.is_empty() {
        tree.push(ast(&mut arena, tokens)?);
    }
    Ok(tree)
}
