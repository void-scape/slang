use bevy_derive::Deref;
use bevy_ecs::{
    prelude::*,
    query::{QueryData, QueryFilter, ROQueryItem},
    system::SystemParam,
};

#[derive(Default, Component)]
pub struct Scope;

// Ident
// ?Variadic
// ?Extern
// Children
//   Args
//   Returns
//   ?Block
#[derive(Component)]
#[require(Scope)]
pub struct Proc {
    pub body: Option<Entity>,
}

#[derive(Component)]
pub struct Extern;

#[derive(Component)]
pub struct Variadic;

#[derive(Component, Deref)]
#[relationship_target(relationship = ArgOf)]
pub struct Args(Vec<Entity>);

// Declaration
#[derive(Component)]
#[relationship(relationship_target = Args)]
pub struct ArgOf(pub Entity);

#[derive(Component, Deref)]
#[relationship_target(relationship = ReturnOf)]
pub struct Returns(Vec<Entity>);

// Type
#[derive(Component)]
#[relationship(relationship_target = Returns)]
pub struct ReturnOf(pub Entity);

#[derive(Component)]
pub struct If {
    pub condition: Entity,
    pub body: Entity,
}

#[derive(Component)]
pub struct While {
    pub condition: Entity,
    pub body: Entity,
}

#[derive(Component)]
pub struct Return {
    pub expr: Option<Entity>,
}

/// Collection of arbitrary code contained in curly braces.
#[derive(Component)]
#[require(Scope)]
pub struct Block;

#[derive(Component)]
pub struct Ident(pub &'static str);

// Type
#[derive(Default, Component)]
pub struct Expr;

#[derive(Component)]
pub struct Variable;

#[derive(Component)]
pub struct VariableOf(pub Entity);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
#[require(Expr)]
pub enum Literal {
    Integer(u64),
    Str(&'static str),
}

// ?Ident
// ?Type
// ?Const
// Children
//   ?Expr
#[derive(Component)]
pub struct Declaration {
    pub expr: Option<Entity>,
}

#[derive(Component)]
pub struct Const;

// Ident
// Children
//   Args
#[derive(Component)]
pub struct Call;

// ?Type
// Children
//   Expr (lhs)
//   Expr (rhs)
#[derive(Clone, Copy, PartialEq, Eq, Component)]
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

// Children
//   Expr
#[derive(Debug, Clone, Copy, Component)]
#[require(Expr)]
pub enum UnaryOp {
    Not,
}

// Layout
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum Type {
    Not,
    U64,
    Str,
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Not => f.write_str("()"),
            Self::U64 => f.write_str("u64"),
            Self::Str => f.write_str("str"),
        }
    }
}

impl Type {
    pub fn is_integer(&self) -> bool {
        matches!(self, Self::U64)
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
    children: Query<'w, 's, &'static Children>,
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

    pub fn iter_descendants_of_parent(
        &self,
        entity: Entity,
    ) -> std::result::Result<impl Iterator<Item = ROQueryItem<'_, 's, D>>, bevy_ecs::error::BevyError>
    {
        let parent = self
            .parents
            .get(entity)
            .map_err(|_| "Entity has no parent")?;
        Ok(self
            .children
            .iter_descendants(parent.0)
            .flat_map(|entity| self.data.get(entity)))
    }
}
