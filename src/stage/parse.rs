use crate::stage::{
    Stage,
    tokenize::{Span, Token, TokenKind, Tokens},
};
use crate::tree::{If, *};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_state::state::OnEnter;
use std::panic::Location;

pub fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(Stage::Parse),
        (parse_files, super::next_stage).chain(),
    );
}

fn parse_files(world: &mut World) -> bevy_ecs::error::Result {
    let tokens = world
        .query::<(Entity, &Tokens)>()
        .iter(world)
        .map(|(e, t)| (e, t.0.clone()))
        .collect::<Vec<_>>();
    for (root, tokens) in tokens.iter() {
        parse(world, &mut tokens.as_slice(), *root)?;
    }
    Ok(())
}

fn parse(world: &mut World, tokens: &mut &[Token], root: Entity) -> Result<()> {
    while !tokens.is_empty() {
        let entity = ast(world, tokens)?;
        world.entity_mut(entity).insert(ChildOf(root));
    }
    Ok(())
}

#[track_caller]
fn parse_type(token: Token) -> Result<Type> {
    let ident = token.ident()?;
    match ident {
        "u8" => Ok(Type::U8),
        "u16" => Ok(Type::U16),
        "u32" => Ok(Type::U32),
        "u64" => Ok(Type::U64),
        //
        "i8" => Ok(Type::I8),
        "i16" => Ok(Type::I16),
        "i32" => Ok(Type::I32),
        "i64" => Ok(Type::I64),
        //
        "str" => Ok(Type::Str),
        _ => Err(Error {
            kind: ErrorKind::InvalidType { ident },
            span: token.span,
            location: Location::caller(),
        }),
    }
}

#[track_caller]
fn ast(world: &mut World, tokens: &mut &[Token]) -> Result<Entity> {
    fn recoverable(result: Result<Entity>) -> Result<Option<Entity>> {
        match result {
            Ok(e) => Ok(Some(e)),
            Err(err) => {
                if err.kind == ErrorKind::Recoverable {
                    Ok(None)
                } else {
                    Err(err)
                }
            }
        }
    }
    for f in [proc, extern_proc, constt, ret, iff, whilee, stmt, block_ast] {
        if let Some(entity) = recoverable(f(world, tokens))? {
            return Ok(entity);
        }
    }
    // TODO: Change this error... what do we expect here?
    Err(tokens[0].error(ErrorKind::Expression {
        got: tokens[0].kind,
    }))
}

fn extern_proc(world: &mut World, tokens: &mut &[Token]) -> Result<Entity> {
    let token = tokens[0];
    token.is_kind_recoverable(TokenKind::Extern)?;
    *tokens = &tokens[1..];
    tokens[0].is_kind(TokenKind::Fn)?;
    let ident = tokens[1];
    let root = world.spawn(Ident(ident.ident()?)).id();
    tokens[2].is_kind(TokenKind::OpenParen)?;
    *tokens = &tokens[2..];
    let end_args = find_matching(tokens, TokenKind::OpenParen, TokenKind::CloseParen)?;
    let (variadic, params) = params(world, &tokens[1..end_args], root)?;
    if variadic {
        world.entity_mut(root).insert(Variadic);
    }
    let mut last_token = tokens[end_args];
    *tokens = &tokens[end_args + 1..];
    let mut return_entity = None;
    if tokens[0].is_kind_recoverable(TokenKind::Arrow).is_ok() {
        let ty = parse_type(tokens[1])?;
        return_entity = Some(
            world
                .spawn((ChildOf(root), RetDecl, ty, tokens[1].span))
                .id(),
        );
        last_token = tokens[1];
        *tokens = &tokens[2..];
    }
    tokens[0].is_kind(TokenKind::Semi)?;
    *tokens = &tokens[1..];
    world.entity_mut(root).insert((
        token.span.collapse(last_token.span),
        Extern,
        Proc,
        Args(params),
        Returns(return_entity),
    ));
    Ok(root)
}

fn proc(world: &mut World, tokens: &mut &[Token]) -> Result<Entity> {
    let token = tokens[0];
    token.is_kind_recoverable(TokenKind::Fn)?;
    let ident = tokens[1];
    let root = world.spawn(Ident(ident.ident()?)).id();
    tokens[2].is_kind(TokenKind::OpenParen)?;
    *tokens = &tokens[2..];
    let end_args = find_matching(tokens, TokenKind::OpenParen, TokenKind::CloseParen)?;
    let (variadic, params) = params(world, &tokens[1..end_args], root)?;
    if variadic {
        world.entity_mut(root).insert(Variadic);
    }
    *tokens = &tokens[end_args + 1..];
    let mut return_entity = None;
    if tokens[0].is_kind_recoverable(TokenKind::Arrow).is_ok() {
        let ty = parse_type(tokens[1])?;
        return_entity = Some(
            world
                .spawn((ChildOf(root), RetDecl, ty, tokens[1].span))
                .id(),
        );
        *tokens = &tokens[2..];
    }
    let (block_entity, body_span) = block(world, tokens)?;
    world.entity_mut(block_entity).insert(ChildOf(root));
    world.entity_mut(root).insert((
        token.span.collapse(body_span),
        Proc,
        Args(params),
        Returns(return_entity),
        Body(block_entity),
    ));
    Ok(root)
}

fn params(world: &mut World, tokens: &[Token], root: Entity) -> Result<(bool, Vec<Entity>)> {
    let mut entities = Vec::new();
    if tokens.is_empty() {
        return Ok((false, entities));
    }
    let start = tokens[0];
    let mut variadic = false;
    for param in tokens.split(|t| t.kind == TokenKind::Comma) {
        if param.len() == 1 && param[0].kind == TokenKind::Variadic {
            variadic = true;
            // TODO: no named after variadic
            continue;
        }
        if param.len() != 3 {
            return Err(start.error(ErrorKind::Declaration { kind: "parameter" }));
        }
        let ident = param[0].ident()?;
        param[1].is_kind(TokenKind::Colon)?;
        let ty = parse_type(param[2])?;
        let span = param[0].span.collapse(param[2].span);
        entities.push(
            world
                .spawn((ChildOf(root), ArgDecl, Ident(ident), ty, span))
                .id(),
        );
    }
    Ok((variadic, entities))
}

fn block(world: &mut World, tokens: &mut &[Token]) -> Result<(Entity, Span)> {
    let entity = world.spawn(Block).id();
    let start = tokens[0];
    // TODO: This is always non recoverable?
    start.is_kind(TokenKind::OpenCurly)?;
    let end = find_matching(tokens, TokenKind::OpenCurly, TokenKind::CloseCurly)?;
    let block = &mut &tokens[1..end];
    let end_span = tokens[end].span;
    *tokens = &tokens[end + 1..];
    while !block.is_empty() {
        let child = ast(world, block)?;
        world.entity_mut(child).insert(ChildOf(entity));
    }
    let span = start.span.collapse(end_span);
    world.entity_mut(entity).insert(span);
    Ok((entity, span))
}

fn block_ast(world: &mut World, tokens: &mut &[Token]) -> Result<Entity> {
    let entity = world.spawn(Block).id();
    let start = tokens[0];
    start.is_kind_recoverable(TokenKind::OpenCurly)?;
    let end = find_matching(tokens, TokenKind::OpenCurly, TokenKind::CloseCurly)?;
    let block = &mut &tokens[1..end];
    let end_span = tokens[end].span;
    *tokens = &tokens[end + 1..];
    while !block.is_empty() {
        let child = ast(world, block)?;
        world.entity_mut(child).insert(ChildOf(entity));
    }
    let span = start.span.collapse(end_span);
    world.entity_mut(entity).insert(span);
    Ok(entity)
}

fn ret(world: &mut World, tokens: &mut &[Token]) -> Result<Entity> {
    let token = tokens[0];
    token.is_kind_recoverable(TokenKind::Return)?;
    *tokens = &tokens[1..];
    let entity = if tokens[0].kind == TokenKind::Semi {
        world.spawn((Return, Type::Not, token.span)).id()
    } else {
        let (expr, span) = bin_op(world, tokens, 0)?;
        let span = token.span.collapse(span);
        let root = world.spawn((Return, span, ReturnExpr(expr))).id();
        world.entity_mut(expr).insert(ChildOf(root));
        root
    };
    tokens[0].is_kind(TokenKind::Semi)?;
    *tokens = &tokens[1..];
    Ok(entity)
}

fn iff(world: &mut World, tokens: &mut &[Token]) -> Result<Entity> {
    let token = tokens[0];
    token.is_kind_recoverable(TokenKind::If)?;
    *tokens = &tokens[1..];
    let (condition, _) = bin_op(world, tokens, 0)?;
    let (body, bspan) = block(world, tokens)?;
    let span = token.span.collapse(bspan);
    let root = world
        .spawn((If, Condition(condition), Body(body), span))
        .id();
    world.entity_mut(condition).insert(ChildOf(root));
    world.entity_mut(body).insert(ChildOf(root));
    Ok(root)
}

fn whilee(world: &mut World, tokens: &mut &[Token]) -> Result<Entity> {
    let token = tokens[0];
    token.is_kind_recoverable(TokenKind::While)?;
    *tokens = &tokens[1..];
    let (condition, _) = bin_op(world, tokens, 0)?;
    let (body, bspan) = block(world, tokens)?;
    let span = token.span.collapse(bspan);
    let root = world
        .spawn((While, Condition(condition), Body(body), span))
        .id();
    world.entity_mut(condition).insert(ChildOf(root));
    world.entity_mut(body).insert(ChildOf(root));
    Ok(root)
}

fn stmt(world: &mut World, tokens: &mut &[Token]) -> Result<Entity> {
    let (expr, _) = bin_op(world, tokens, 0)?;
    tokens[0].is_kind(TokenKind::Semi)?;
    *tokens = &tokens[1..];
    Ok(expr)
}

fn constt(world: &mut World, tokens: &mut &[Token]) -> Result<Entity> {
    let token = tokens[0];
    token.is_kind_recoverable(TokenKind::Const)?;
    *tokens = &tokens[1..];
    let (decl, _) = var_decl(world, tokens)?;
    tokens[0].is_kind(TokenKind::Semi)?;
    *tokens = &tokens[1..];
    world.entity_mut(decl).insert(Const);
    Ok(decl)
}

impl Token {
    #[track_caller]
    fn is_kind(&self, kind: TokenKind) -> Result<()> {
        (self.kind == kind).ok_or(self.error(ErrorKind::Expected {
            expected: kind,
            got: Some(self.kind),
        }))
    }

    fn is_kind_recoverable(&self, kind: TokenKind) -> Result<()> {
        (self.kind == kind).ok_or_else(Error::recoverable)
    }

    #[track_caller]
    fn ident(&self) -> Result<&'static str> {
        match self.kind {
            TokenKind::Ident(ident) => Ok(ident),
            token => Err(self.error(ErrorKind::ExpectedIdent { got: token })),
        }
    }
}

#[track_caller]
fn find_matching(
    tokens: &[Token],
    open: TokenKind,
    close: TokenKind,
) -> std::result::Result<usize, Error> {
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
        .ok_or(tokens[0].error(ErrorKind::UnmatchedDelimiter {
            delimiter: tokens[0].kind,
        }))
}

fn var_decl(world: &mut World, tokens: &mut &[Token]) -> Result<(Entity, Span)> {
    let start_span = tokens[0].span;
    let ident = Ident(tokens[0].ident()?);
    let ty = match tokens[1].kind {
        TokenKind::Equals => {
            *tokens = &tokens[2..];
            None
        }
        TokenKind::Colon => {
            let ty = parse_type(tokens[2])?;
            tokens[3].is_kind(TokenKind::Equals)?;
            *tokens = &tokens[4..];
            Some(ty)
        }
        got => {
            return Err(tokens[1].error(ErrorKind::Expected {
                expected: TokenKind::Equals,
                got: Some(got),
            }));
        }
    };
    let (expr, espan) = bin_op(world, tokens, 0)?;
    let span = start_span.collapse(espan);
    let root = world.spawn((VarDecl, DeclExpr(expr), ident, span)).id();
    if let Some(ty) = ty {
        world.entity_mut(root).insert(ty);
    }
    world.entity_mut(expr).insert(ChildOf(root));
    Ok((root, span))
}

fn args(world: &mut World, tokens: &[Token], root: Entity) -> Result<Vec<Entity>> {
    let mut args = Vec::new();
    if tokens.is_empty() {
        return Ok(args);
    }
    for mut arg_tokens in tokens.split(|t| t.kind == TokenKind::Comma) {
        let (entity, _) = bin_op(world, &mut arg_tokens, 0)?;
        world.entity_mut(entity).insert(ChildOf(root));
        args.push(entity);
    }
    Ok(args)
}

fn expr(world: &mut World, tokens: &mut &[Token]) -> Result<(Entity, Span)> {
    let token = tokens[0];
    match token.kind {
        TokenKind::Let => {
            *tokens = &tokens[1..];
            let (entity, span) = var_decl(world, tokens)?;
            Ok((entity, token.span.collapse(span)))
        }
        TokenKind::Ident(ident) => {
            *tokens = &tokens[1..];
            let root = world.spawn_empty().id();
            if tokens
                .first()
                .is_some_and(|t| t.kind == TokenKind::OpenParen)
            {
                let end_args = find_matching(tokens, TokenKind::OpenParen, TokenKind::CloseParen)?;
                let args = args(world, &tokens[1..end_args], root)?;
                let end_span = tokens[end_args].span;
                *tokens = &tokens[end_args + 1..];
                let span = token.span.collapse(end_span);
                world
                    .entity_mut(root)
                    .insert((Call, Ident(ident), CallArgs(args), span));
                Ok((root, span))
            } else {
                let span = token.span;
                world
                    .entity_mut(root)
                    .insert((Ident(ident), Variable, span));
                Ok((root, span))
            }
        }
        TokenKind::Integer(integer) => {
            *tokens = &tokens[1..];
            let span = token.span;
            let entity = world.spawn((Literal::Integer(integer), span)).id();
            Ok((entity, span))
        }
        TokenKind::Str(str) => {
            *tokens = &tokens[1..];
            let span = token.span;
            let entity = world.spawn((Literal::Str(str), span)).id();
            Ok((entity, span))
        }
        TokenKind::Not => {
            *tokens = &tokens[1..];
            let (expr, espan) = expr(world, tokens)?;
            let span = token.span.collapse(espan);
            let root = world.spawn((UnaryOp::Not, span)).id();
            world.entity_mut(expr).insert(ChildOf(root));
            Ok((root, span))
        }
        TokenKind::OpenParen => {
            *tokens = &tokens[1..];
            let (entity, span) = bin_op(world, tokens, 0)?;
            tokens[0].is_kind(TokenKind::CloseParen)?;
            *tokens = &tokens[1..];
            Ok((entity, span))
        }
        got => Err(tokens[0].error(ErrorKind::Expression { got })),
    }
}

impl BinOp {
    // Precedence according to the rust standard, of which I am familiar with:
    // https://doc.rust-lang.org/reference/expressions.html
    const TABLE: &[&[BinOp]] = &[
        &[
            BinOp::Assign,
            BinOp::AddAssign,
            BinOp::SubAssign,
            BinOp::MulAssign,
            BinOp::DivAssign,
            BinOp::ModAssign,
            BinOp::BitAndAssign,
            BinOp::BitOrAssign,
            BinOp::XorAssign,
            BinOp::ShlAssign,
            BinOp::ShrAssign,
        ],
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
fn bin_op(world: &mut World, tokens: &mut &[Token], precedence: usize) -> Result<(Entity, Span)> {
    if precedence > BinOp::max_precedence() {
        return expr(world, tokens);
    }

    let mut lhs = bin_op(world, tokens, precedence + 1)?;
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
            let rhs = bin_op(world, tokens, precedence + 1)?;
            let new_span = lhs.1.collapse(rhs.1);
            let new_entity = world.spawn((op, new_span)).id();
            world.entity_mut(lhs.0).insert(ChildOf(new_entity));
            world.entity_mut(rhs.0).insert(ChildOf(new_entity));
            lhs = (new_entity, new_span);
            saved = *tokens;
        }
    }

    *tokens = saved;
    Ok(lhs)
}

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

impl Token {
    #[track_caller]
    fn error(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            span: self.span,
            location: Location::caller(),
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
    InvalidType {
        ident: &'static str,
    },
    ExpectedIdent {
        got: TokenKind,
    },
    UnmatchedDelimiter {
        delimiter: TokenKind,
    },
    Declaration {
        kind: &'static str,
    },
    Expression {
        got: TokenKind,
    },
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
            ErrorKind::InvalidType { ident } => f.write_str(&format!("Invalid type `{ident}`")),
            ErrorKind::ExpectedIdent { got } => {
                f.write_str(&format!("Expected identifier, got `{got}`"))
            }
            ErrorKind::UnmatchedDelimiter { delimiter } => {
                f.write_str(&format!("Unmatched delimiter `{delimiter}`"))
            }
            ErrorKind::Declaration { kind } => f.write_str(&format!("Invalid {kind} declaration")),
            ErrorKind::Expression { got } => f.write_str(&format!("Invalid expression `{got}`")),
        }
    }
}
