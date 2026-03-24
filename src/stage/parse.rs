use crate::stage::tokenize::Span;
use crate::tree::{If, *};
use crate::{
    error::Report,
    stage::{
        Stage,
        tokenize::{Token, TokenKind, Tokens},
    },
};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_state::state::OnEnter;

type Result<T> = std::result::Result<T, Error>;
enum Error {
    Cut(crate::error::Error),
    Recoverable(Span),
}
impl From<crate::error::Error> for Error {
    fn from(value: crate::error::Error) -> Self {
        Self::Cut(value)
    }
}
impl Error {
    fn into_reportable_error(self) -> crate::error::Error {
        match self {
            Self::Cut(cut) => cut,
            Self::Recoverable(span) => span.custom(
                "Recoverable error ignored, \
                this is a bug!",
            ),
        }
    }
}

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
        if tokens.is_empty() {
            continue;
        }
        let mut parser = Parser {
            world,
            tokens: &mut tokens.as_slice(),
            last: tokens[0].span,
        };
        // NOTE: `any` will return an error if it can't parse the input, so this
        // will never hang.
        while !parser.tokens.is_empty() {
            let entity = parser.any().map_err(|err| err.into_reportable_error())?;
            parser.make_child(*root, entity);
        }
    }
    Ok(())
}

// COMMON CONSTRUCTS

impl<'p> Parser<'p> {
    fn any_of(
        &mut self,
        fs: &[fn(&mut Parser<'p>) -> Result<Entity>],
        err: &'static str,
    ) -> Result<Entity> {
        for f in fs.iter() {
            match f(self) {
                Ok(entity) => {
                    return Ok(entity);
                }
                Err(Error::Cut(cut)) => return Err(Error::Cut(cut)),
                Err(Error::Recoverable(_)) => {}
            }
        }
        Err(self.last.custom(err).into())
    }

    fn any(&mut self) -> Result<Entity> {
        self.any_of(
            &[
                Self::proc_recoverable,
                Self::extern_proc_recoverable,
                Self::const_recoverable,
                Self::let_recoverable,
                Self::return_recoverable,
                Self::if_recoverable,
                Self::while_recoverable,
                Self::block_recoverable,
                // NOTE: This is not recoverable because garbage might be
                // spawned by `bin_op`, so we can't just continue.
                Self::bin_op_stmt,
            ],
            "How did you get here ;-)",
        )
    }

    fn ty(&mut self) -> Result<(Span, Type)> {
        let (span, ident) = self.eat_ident_spanned()?;
        let ty = match ident.0 {
            "u8" => Type::U8,
            "u16" => Type::U16,
            "u32" => Type::U32,
            "u64" => Type::U64,
            //
            "i8" => Type::I8,
            "i16" => Type::I16,
            "i32" => Type::I32,
            "i64" => Type::I64,
            //
            "str" => Type::Str,
            _ => return Err(span.custom("Undeclared type").into()),
        };
        Ok((span, ty))
    }

    fn block_recoverable(&mut self) -> Result<Entity> {
        if self.peek().is_some_and(|t| t.kind == TokenKind::OpenCurly) {
            self.block()
        } else {
            Err(self.eat_kind_recoverable(TokenKind::OpenCurly).unwrap_err())
        }
    }

    fn block(&mut self) -> Result<Entity> {
        let root = self.new_entity();
        let start = self.eat_kind(TokenKind::OpenCurly)?;
        while self.peek().is_some_and(|t| t.kind != TokenKind::CloseCurly) {
            let child = self.any()?;
            self.make_child(root, child);
        }
        let end = self.eat_kind(TokenKind::CloseCurly)?;
        self.insert(root, Block);
        self.insert_spanned(root, start.span, end.span);
        Ok(root)
    }
}

// PROCEDURES

impl Parser<'_> {
    fn proc_recoverable(&mut self) -> Result<Entity> {
        let start = self.eat_kind_recoverable(TokenKind::Fn)?;
        let root = self.proc_sig()?;
        let body = self.block()?;
        let body_span = self.span(body);
        self.make_child(root, body);
        self.insert_spanned(root, start.span, body_span);
        self.insert(root, Body(body));
        Ok(root)
    }

    fn extern_proc_recoverable(&mut self) -> Result<Entity> {
        let start = self.eat_kind_recoverable(TokenKind::Extern)?;
        self.eat_kind(TokenKind::Fn)?;
        let root = self.proc_sig()?;
        let end = self.eat_kind(TokenKind::Semi)?;
        self.insert_spanned(root, start.span, end.span);
        self.insert(root, Extern);
        Ok(root)
    }

    fn proc_sig(&mut self) -> Result<Entity> {
        let ident = self.eat_ident()?;
        let root = self.new_entity();

        let mut args = Vec::new();
        self.eat_kind(TokenKind::OpenParen)?;
        let mut first = true;
        while self.peek().is_some_and(|t| t.kind != TokenKind::CloseParen) {
            if !first {
                self.eat_kind(TokenKind::Comma)?;
            }
            if self.eat_kind_recoverable(TokenKind::Variadic).is_ok() {
                self.insert(root, Variadic);
                break;
            }
            let (start, ident) = self.eat_ident_spanned()?;
            self.eat_kind(TokenKind::Colon)?;
            let (end, ty) = self.ty()?;
            let arg = self.world.spawn((ChildOf(root), ArgDecl, ident, ty)).id();
            self.insert_spanned(arg, start, end);
            args.push(arg);
            first = false;
        }
        self.eat_kind(TokenKind::CloseParen)?;

        self.insert(root, (Proc, ident, Args(args), Returns(None)));
        if self.eat_kind_recoverable(TokenKind::Arrow).is_ok() {
            let (span, ty) = self.ty()?;
            let entity = self.world.spawn((ChildOf(root), RetDecl, ty, span)).id();
            self.insert(root, Returns(Some(entity)));
        }

        Ok(root)
    }
}

// CONTROL FLOW

impl Parser<'_> {
    fn condition_and_body_recoverable(
        &mut self,
        token: TokenKind,
        bundle: impl Bundle,
    ) -> Result<Entity> {
        let start = self.eat_kind_recoverable(token)?;
        let root = self.new_entity();

        let condition = self.bin_op()?;
        let body = self.block()?;
        self.make_child(root, condition);
        self.make_child(root, body);

        self.insert(root, (bundle, Condition(condition), Body(body)));
        let end = self.span(body);
        self.insert_spanned(root, start.span, end);
        Ok(root)
    }

    fn if_recoverable(&mut self) -> Result<Entity> {
        self.condition_and_body_recoverable(TokenKind::If, If)
    }

    fn while_recoverable(&mut self) -> Result<Entity> {
        self.condition_and_body_recoverable(TokenKind::While, While)
    }

    fn return_recoverable(&mut self) -> Result<Entity> {
        let start = self.eat_kind_recoverable(TokenKind::Return)?;
        if self.peek().is_some_and(|t| t.kind == TokenKind::Semi) {
            let end = self.eat_kind(TokenKind::Semi)?;
            let root = self.world.spawn((Return, Type::Not)).id();
            self.insert_spanned(root, start.span, end.span);
            Ok(root)
        } else {
            let expr = self.bin_op()?;
            let root = self.world.spawn((Return, ReturnExpr(expr))).id();
            self.make_child(root, expr);
            let end = self.eat_kind(TokenKind::Semi)?;
            self.insert_spanned(root, start.span, end.span);
            Ok(root)
        }
    }
}

// STATEMENTS

impl Parser<'_> {
    // NOTE: not really a statement?
    fn var_decl(&mut self) -> Result<Entity> {
        let (start, ident) = self.eat_ident_spanned()?;
        let root = self.new_entity();

        if self.eat_kind_recoverable(TokenKind::Colon).is_ok() {
            let (_, ty) = self.ty()?;
            self.insert(root, ty);
        }

        self.eat_kind(TokenKind::Equals)?;
        let expr = self.bin_op()?;
        self.make_child(root, expr);

        self.insert(root, (VarDecl, ident, DeclExpr(expr)));
        let end = self.span(expr);
        self.insert_spanned(root, start, end);

        Ok(root)
    }

    fn let_recoverable(&mut self) -> Result<Entity> {
        let start = self.eat_kind_recoverable(TokenKind::Let)?;
        let decl = self.var_decl()?;
        let end = self.eat_kind(TokenKind::Semi)?;
        self.insert_spanned(decl, start.span, end.span);
        Ok(decl)
    }

    fn const_recoverable(&mut self) -> Result<Entity> {
        let start = self.eat_kind_recoverable(TokenKind::Const)?;
        let decl = self.var_decl()?;
        let end = self.eat_kind(TokenKind::Semi)?;
        self.insert_spanned(decl, start.span, end.span);
        self.insert(decl, Const);
        Ok(decl)
    }

    fn bin_op_stmt(&mut self) -> Result<Entity> {
        let expr = self.bin_op()?;
        let start = self.span(expr);
        let end = self.eat_kind(TokenKind::Semi)?;
        self.insert_spanned(expr, start, end.span);
        Ok(expr)
    }
}

// EXPRESSIONS

impl Parser<'_> {
    fn expr(&mut self) -> Result<Entity> {
        self.any_of(
            &[
                Self::call,
                Self::variable,
                Self::integer,
                Self::str,
                Self::not,
                Self::paren,
            ],
            "Invalid expression",
        )
    }

    fn call(&mut self) -> Result<Entity> {
        // NOTE: Recoverable because this might just be a variable, in which case
        // `variable` will return the ident as a variable.
        if self.tokens.len() < 2 || self.tokens[1].kind != TokenKind::OpenParen {
            return Err(Error::Recoverable(self.last));
        }

        let (start, ident) = self.eat_ident_spanned_recoverable()?;
        let mut args = Vec::new();
        self.eat_kind(TokenKind::OpenParen)?;
        let root = self.new_entity();
        let mut first = true;
        while self.peek().is_some_and(|t| t.kind != TokenKind::CloseParen) {
            if !first {
                self.eat_kind(TokenKind::Comma)?;
            }
            let expr = self.bin_op()?;
            self.make_child(root, expr);
            args.push(expr);
            first = false;
        }
        let end = self.eat_kind(TokenKind::CloseParen)?;
        self.insert(root, (Call, ident, CallArgs(args)));
        self.insert_spanned(root, start, end.span);
        Ok(root)
    }

    fn variable(&mut self) -> Result<Entity> {
        let (span, ident) = self.eat_ident_spanned_recoverable()?;
        Ok(self.world.spawn((ident, Variable, span)).id())
    }

    fn integer(&mut self) -> Result<Entity> {
        let (span, integer) =
            self.eat_fn_recoverable(integer_spanned, "Expected integer literal")?;
        Ok(self.world.spawn((Literal::Integer(integer), span)).id())
    }

    fn str(&mut self) -> Result<Entity> {
        let (span, str) = self.eat_fn_recoverable(str_spanned, "Expected integer literal")?;
        Ok(self.world.spawn((Literal::Str(str), span)).id())
    }

    fn not(&mut self) -> Result<Entity> {
        let start = self.eat_kind_recoverable(TokenKind::Bang)?;
        let expr = self.expr()?;
        let root = self.world.spawn(UnaryOp::Not).id();
        let end = self.span(expr);
        self.insert_spanned(root, start.span, end);
        self.make_child(root, expr);
        Ok(root)
    }

    fn paren(&mut self) -> Result<Entity> {
        let start = self.eat_kind_recoverable(TokenKind::OpenParen)?;
        let expr = self.bin_op()?;
        let end = self.eat_kind(TokenKind::CloseParen)?;
        self.insert_spanned(expr, start.span, end.span);
        Ok(expr)
    }
}

fn ident_spanned(token: Token) -> Result<(Span, Ident)> {
    match token.kind {
        TokenKind::Ident(ident) => Ok((token.span, Ident(ident))),
        _ => Err(token.span.custom("Expected identifier").into()),
    }
}

fn integer_spanned(token: Token) -> Result<(Span, u64)> {
    match token.kind {
        TokenKind::Integer(integer) => Ok((token.span, integer)),
        _ => Err(token.span.custom("Expected integer literal").into()),
    }
}

fn str_spanned(token: Token) -> Result<(Span, &'static str)> {
    match token.kind {
        TokenKind::Str(str) => Ok((token.span, str)),
        _ => Err(token.span.custom("Expected string literal").into()),
    }
}

// BINARY OPERATIONS

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

impl Parser<'_> {
    fn bin_op(&mut self) -> Result<Entity> {
        bin_op(self, 0)
    }
}

// Stolen from tsoding's B compiler:
// https://github.com/bext-lang/b/blob/main/src/b.rs#L515
fn bin_op(parser: &mut Parser, precedence: usize) -> Result<Entity> {
    if precedence > BinOp::max_precedence() {
        return parser.expr();
    }

    let mut lhs = bin_op(parser, precedence + 1)?;
    let mut saved = (*parser.tokens, parser.last);

    if !parser.tokens.is_empty()
        && let Some(op) = BinOp::from_token(parser.tokens[0].kind)
        && op.precedence() == precedence
    {
        while !parser.tokens.is_empty()
            && let Some(op) = BinOp::from_token(parser.tokens[0].kind)
            && op.precedence() == precedence
        {
            parser.eat();
            let rhs = bin_op(parser, precedence + 1)?;
            // let new_span = lhs.1.collapse(rhs.1);
            let new_entity = parser.world.spawn(op).id();
            let rhs_span = parser.span(rhs);
            let lhs_span = parser.span(lhs);
            parser.insert_spanned(new_entity, lhs_span, rhs_span);
            parser.make_child(new_entity, lhs);
            parser.make_child(new_entity, rhs);
            lhs = new_entity;
            saved = (*parser.tokens, parser.last);
        }
    }

    *parser.tokens = saved.0;
    parser.last = saved.1;
    Ok(lhs)
}

// PARSING UTILITY

struct Parser<'a> {
    world: &'a mut World,
    tokens: &'a mut &'a [Token],
    last: Span,
}

impl Parser<'_> {
    fn eat(&mut self) -> Option<Token> {
        if !self.tokens.is_empty() {
            let token = self.tokens[0];
            *self.tokens = &self.tokens[1..];
            self.last = token.span;
            Some(token)
        } else {
            None
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.first()
    }

    fn eat_kind(&mut self, kind: TokenKind) -> Result<Token> {
        // NOTE: Complicated expression here because we dont want to be formatting
        // strings for lots of tokens!
        Ok(self
            .eat()
            .map(|token| {
                (token.kind == kind)
                    .then_some(token)
                    .ok_or_else(|| token.span.msg(format!("Expected `{kind}`")))
            })
            .ok_or_else(|| self.last.msg(format!("Expected `{kind}`")))??)
    }

    fn eat_kind_recoverable(&mut self, kind: TokenKind) -> Result<Token> {
        if self.peek().is_some_and(|t| t.kind == kind) {
            Ok(self.eat().unwrap())
        } else {
            Err(Error::Recoverable(self.last))
        }
    }

    fn eat_fn<R>(&mut self, f: impl Fn(Token) -> Result<R>, err: &'static str) -> Result<R> {
        match self.eat() {
            Some(token) => f(token),
            None => Err(self.last.custom(err).into()),
        }
    }

    fn eat_fn_recoverable<R>(
        &mut self,
        f: impl Fn(Token) -> Result<R>,
        err: &'static str,
    ) -> Result<R> {
        let chk = (*self.tokens, self.last);
        match self.eat_fn(f, err) {
            Ok(r) => Ok(r),
            Err(_) => {
                *self.tokens = chk.0;
                self.last = chk.1;
                Err(Error::Recoverable(self.last))
            }
        }
    }

    fn eat_ident_spanned(&mut self) -> Result<(Span, Ident)> {
        self.eat_fn(ident_spanned, "Expected identifier")
    }

    fn eat_ident_spanned_recoverable(&mut self) -> Result<(Span, Ident)> {
        self.eat_fn_recoverable(ident_spanned, "Expected identifier")
    }

    fn eat_ident(&mut self) -> Result<Ident> {
        Ok(self.eat_ident_spanned()?.1)
    }

    fn new_entity(&mut self) -> Entity {
        self.world.spawn_empty().id()
    }

    fn insert(&mut self, entity: Entity, bundle: impl Bundle) {
        self.world.entity_mut(entity).insert(bundle);
    }

    fn span(&self, entity: Entity) -> Span {
        *self.world.entity(entity).get::<Span>().unwrap()
    }

    fn insert_spanned(&mut self, entity: Entity, start: Span, end: Span) {
        debug_assert_eq!(start.location, end.location);
        self.insert(
            entity,
            Span {
                start: start.start.min(end.start),
                end: start.end.max(end.end),
                location: start.location,
            },
        );
    }

    fn make_child(&mut self, root: Entity, child: Entity) {
        self.world.entity_mut(child).insert(ChildOf(root));
    }
}
