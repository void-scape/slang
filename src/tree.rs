use bevy_derive::Deref;
use bevy_ecs::{
    lifecycle::HookContext,
    prelude::*,
    query::{QueryData, QueryFilter, ROQueryItem},
    system::SystemParam,
    world::DeferredWorld,
};

/// Marker for opening a new variable scope.
///
/// If an entity contains [`Scope`], then every variable defined within it
/// can not be accessed by parent [`Scope`]s.
#[derive(Default, Component)]
pub struct Scope;

/// Marker for an expression that requires a [`Type`].
///
/// A [`Type`] will be assigned to [`Typed`] entities in
/// [`Stage::Type`](crate::stage::Stage::Type).
#[derive(Default, Component)]
pub struct Typed;

/// Marker for a [`Typed`] entity that assigns its parent its own [`Type`].
#[derive(Default, Component)]
pub struct TypePropagatesUp;

/// Prevents [`TypePropagatesUp`] from assign this type.
#[derive(Default, Component)]
pub struct DontPropagateUpTo;

/// Marker for a [`Typed`] entity that assigns its children its own [`Type`].
#[derive(Default, Component)]
pub struct TypePropagatesDown;

#[derive(Debug, Component)]
pub struct Ident(pub &'static str);

impl std::fmt::Display for Ident {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

// Block components

/// Collection of arbitrary code contained in curly braces.
#[derive(Component)]
#[require(Scope)]
pub struct Block;

/// Points to a [`Block`] entity.
#[derive(Component)]
pub struct Body(pub Entity);

// Procedure components

/// Loose entity structure:
/// - [`Args`]
/// - [`Returns`]
/// - ?[`Body`]
/// - ?[`Extern`]
/// - ?[`Variadic`]
#[derive(Component)]
#[require(Scope, Typed)]
pub struct Proc;

#[derive(Component)]
pub struct Extern;

#[derive(Component)]
pub struct Variadic;

/// The collection of [`Proc`] argument [`ArgDecl`]s.
#[derive(Component, Deref)]
pub struct Args(pub Vec<Entity>);

/// An optional [`Proc`] [`RetDecl`].
#[derive(Component, Deref)]
pub struct Returns(pub Option<Entity>);

// Control flow components

/// An [`Expr`] that produces an integer value.
#[derive(Component)]
pub struct Condition(pub Entity);

/// Loose entity structure:
/// - [`Condition`]
/// - [`Body`]
#[derive(Component)]
pub struct If;

/// Loose entity structure:
/// - [`Condition`]
/// - [`Body`]
#[derive(Component)]
pub struct While;

/// Loose entity structure:
/// - ?[`ReturnExpr`]
#[derive(Default, Component)]
#[require(Typed, TypePropagatesDown)]
pub struct Return;

/// Points to an [`Expr`] that returns a value to the parent [`Proc`].
#[derive(Component)]
pub struct ReturnExpr(pub Entity);

// Expression components

/// Reference to an [`ArgDecl`] or [`VarDecl`].
///
/// All [`Variable`]s must be assigned a [`VariableOf`] relationship in
/// [`Stage::Scope`](crate::stage::Stage::Scope).
///
/// Loose entity structure:
/// - [`VariableOf`]
#[derive(Component)]
#[require(Typed, TypePropagatesUp)]
pub struct Variable;

#[derive(Component)]
pub struct VariableOf(pub Entity);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
#[require(Typed, TypePropagatesUp)]
pub enum Literal {
    Integer(u64),
    Str(&'static str),
}

#[derive(Component)]
pub struct DeclExpr(pub Entity);

/// Shared between [`VarDecl`] and [`ArgDecl`].
#[derive(Default, Component)]
pub struct NamedDecl;

/// Shared between [`VarDecl`], [`ArgDecl`], and [`RetDecl`].
#[derive(Default, Component)]
pub struct Decl;

/// Loose entity structure:
/// - [`Ident`]
/// - [`DeclExpr`]
/// - ?[`Const`]
#[derive(Component)]
#[require(Decl, NamedDecl, Typed, TypePropagatesDown)]
pub struct VarDecl;

/// Marker for a constant [`VarDecl`].
///
/// [`Const`] values are folded at compile time in
/// [`Stage::Fold`](crate::stage::Stage::Fold).
#[derive(Component)]
pub struct Const;

/// Loose entity structure:
/// - [`Ident`]
/// - [`Type`]
#[derive(Component)]
#[require(Decl, NamedDecl)]
pub struct ArgDecl;

/// Defines the return type of a [`Proc`].
///
/// Loose entity structure:
/// - [`Type`]
#[derive(Component)]
#[require(Decl)]
pub struct RetDecl;

/// Loose entity structure:
/// - [`Ident`]
/// - [`CallArgs`]
#[derive(Component)]
#[require(Typed, DontPropagateUpTo, TypePropagatesUp)]
pub struct Call;

/// The collection of [`Call`] argument [`Expr`]s.
#[derive(Component, Deref)]
pub struct CallArgs(pub Vec<Entity>);

/// Loose entity structure:
/// - [`Children`]
/// - ?[`Logical`]
/// - ?[`Assignment`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
#[require(Typed, TypePropagatesDown, TypePropagatesUp)]
#[component(on_insert = Self::classify)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    //
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    //
    And,
    Or,
    //
    BitAnd,
    BitOr,
    Xor,
    Shr,
    Shl,
    //
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    BitAndAssign,
    BitOrAssign,
    XorAssign,
    ShlAssign,
    ShrAssign,
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Add => f.write_str("+"),
            Self::Sub => f.write_str("-"),
            Self::Mul => f.write_str("*"),
            Self::Div => f.write_str("/"),
            Self::Mod => f.write_str("%"),
            Self::Eq => f.write_str("=="),
            Self::Ne => f.write_str("!="),
            Self::Gt => f.write_str(">"),
            Self::Ge => f.write_str(">="),
            Self::Lt => f.write_str("<"),
            Self::Le => f.write_str("<="),
            Self::And => f.write_str("&&"),
            Self::Or => f.write_str("||"),
            Self::BitAnd => f.write_str("&"),
            Self::BitOr => f.write_str("|"),
            Self::Xor => f.write_str("^"),
            Self::Shr => f.write_str(">>"),
            Self::Shl => f.write_str("<<"),
            Self::Assign => f.write_str("="),
            Self::AddAssign => f.write_str("+="),
            Self::SubAssign => f.write_str("-="),
            Self::MulAssign => f.write_str("*="),
            Self::DivAssign => f.write_str("/="),
            Self::ModAssign => f.write_str("%="),
            Self::BitAndAssign => f.write_str("&="),
            Self::BitOrAssign => f.write_str("|="),
            Self::XorAssign => f.write_str("^="),
            Self::ShlAssign => f.write_str(">>="),
            Self::ShrAssign => f.write_str("<<="),
        }
    }
}

impl BinOp {
    fn classify(mut world: DeferredWorld, ctx: HookContext) {
        let op = world.get::<Self>(ctx.entity).unwrap();
        if matches!(
            op,
            Self::Assign
                | Self::AddAssign
                | Self::SubAssign
                | Self::MulAssign
                | Self::DivAssign
                | Self::ModAssign
                | Self::BitAndAssign
                | Self::BitOrAssign
                | Self::XorAssign
                | Self::ShlAssign
                | Self::ShrAssign
        ) {
            world.commands().entity(ctx.entity).insert(Assignment);
        } else if matches!(op, Self::And | Self::Or) {
            world.commands().entity(ctx.entity).insert(Logical);
        }
    }
}

/// Marks a logical operation ([`BinOp::And`], [`BinOp::Or`]).
#[derive(Component)]
pub struct Logical;

/// Marks an assignment operation.
#[derive(Component)]
pub struct Assignment;

/// Loose entity structure:
/// - [`Children`]
#[derive(Debug, Clone, Copy, Component)]
#[require(Typed, TypePropagatesDown, TypePropagatesUp)]
pub enum UnaryOp {
    Not,
}

/// [`Layout`] is generated for a [`Type`] in
/// [`Stage::Ir`](crate::stage::Stage::Ir).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum Type {
    Not,
    Str,
    //
    U8,
    U16,
    U32,
    U64,
    //
    I8,
    I16,
    I32,
    I64,
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Not => f.write_str("()"),
            Self::Str => f.write_str("str"),
            //
            Self::U8 => f.write_str("u8"),
            Self::U16 => f.write_str("u16"),
            Self::U32 => f.write_str("u32"),
            Self::U64 => f.write_str("u64"),
            //
            Self::I8 => f.write_str("i8"),
            Self::I16 => f.write_str("i16"),
            Self::I32 => f.write_str("i32"),
            Self::I64 => f.write_str("i64"),
        }
    }
}

impl Type {
    pub fn is_integer(&self) -> bool {
        match self {
            Self::U8
            | Self::U16
            | Self::U32
            | Self::U64
            | Self::I8
            | Self::I16
            | Self::I32
            | Self::I64 => true,
            Self::Not | Self::Str => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Component)]
pub struct Layout {
    pub size: usize,
    pub align: usize,
    pub composite: bool,
}

#[derive(SystemParam)]
pub struct TreeQuery<'w, 's, D, F = ()>
where
    D: QueryData + 'static,
    F: QueryFilter + 'static,
{
    parents: Query<'w, 's, &'static ChildOf>,
    data: Query<'w, 's, D, F>,
}

impl<'s, D, F> TreeQuery<'_, 's, D, F>
where
    D: QueryData + 'static,
    F: QueryFilter + 'static,
{
    pub fn first_ancestor(
        &self,
        entity: Entity,
    ) -> bevy_ecs::error::Result<ROQueryItem<'_, 's, D>> {
        for e in self.parents.iter_ancestors(entity) {
            if let Ok(result) = self.data.get(e) {
                return Ok(result);
            }
        }
        Err(format!("No ancestors with `{}`", std::any::type_name::<D>()).into())
    }
}
