use crate::tokenize::Span;

pub trait Spanned {
    fn span(&self) -> Span;
}

impl<T: Spanned> Spanned for &T {
    fn span(&self) -> Span {
        <T as Spanned>::span(self)
    }
}

macro_rules! spanned {
    ($ident:ident) => {
        impl Spanned for $ident {
            fn span(&self) -> Span {
                self.span
            }
        }
    };
}

#[derive(Debug, Clone, Copy)]
pub enum Ast {
    Func(Func),
    Block(Block),
    Expr(Expr),
    Return(Return),
    If(If),
    While(While),
}

impl Spanned for Ast {
    fn span(&self) -> Span {
        match self {
            Self::Func(func) => func.span(),
            Self::Block(block) => block.span(),
            Self::Expr(expr) => expr.span(),
            Self::Return(ret) => ret.span(),
            Self::If(iff) => iff.span(),
            Self::While(whilee) => whilee.span(),
        }
    }
}

spanned!(Func);
#[derive(Debug, Clone, Copy)]
pub struct Func {
    pub span: Span,
    pub ident: Ident,
    pub arguments: &'static [Declaration],
    pub returns: &'static [Declaration],
    pub body: Block,
    pub variadic: bool,
}

spanned!(Block);
#[derive(Debug, Clone, Copy)]
pub struct Block {
    pub span: Span,
    pub statements: &'static [Ast],
}

#[derive(Debug, Clone, Copy)]
pub enum Expr {
    Ident(Ident),
    Literal(Literal),
    BinOp(BinOp),
    UnaryOp(UnaryOp),
    Declaration(Declaration),
    Call(Call),
}

impl Spanned for Expr {
    fn span(&self) -> Span {
        match self {
            Self::Ident(ident) => ident.span(),
            Self::Literal(literal) => literal.span(),
            Self::BinOp(bin) => bin.span(),
            Self::UnaryOp(unary) => unary.span(),
            Self::Declaration(decl) => decl.span(),
            Self::Call(call) => call.span(),
        }
    }
}

spanned!(Ident);
#[derive(Debug, Default, Clone, Copy)]
pub struct Ident {
    pub span: Span,
    pub value: &'static str,
}

spanned!(Literal);
#[derive(Debug, Clone, Copy)]
pub struct Literal {
    pub span: Span,
    pub kind: LiteralKind,
}

#[derive(Debug, Clone, Copy)]
pub enum LiteralKind {
    Integer(u64),
    Str(&'static str),
}

spanned!(BinOp);
#[derive(Debug, Clone, Copy)]
pub struct BinOp {
    pub span: Span,
    pub kind: BinOpKind,
    pub lhs: &'static Expr,
    pub rhs: &'static Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOpKind {
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

spanned!(UnaryOp);
#[derive(Debug, Clone, Copy)]
pub struct UnaryOp {
    pub span: Span,
    pub kind: UnaryOpKind,
    pub expr: &'static Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOpKind {
    Not,
}

spanned!(Declaration);
#[derive(Debug, Clone, Copy)]
pub struct Declaration {
    pub span: Span,
    pub ident: Ident,
    pub ty: Option<Type>,
    pub rhs: Option<&'static Expr>,
}

#[derive(Debug, Clone, Copy)]
pub struct Type {
    pub kind: TypeKind,
    pub layout: Layout,
}

impl Type {
    pub fn new(kind: TypeKind) -> Self {
        Self {
            kind,
            layout: kind.layout(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    U64,
}

impl TypeKind {
    pub fn layout(&self) -> Layout {
        match self {
            Self::U64 => Layout {
                size: 8,
                align: 8,
                composite: false,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Layout {
    pub size: usize,
    pub align: usize,
    pub composite: bool,
}

spanned!(Call);
#[derive(Debug, Clone, Copy)]
pub struct Call {
    pub span: Span,
    pub ident: Ident,
    pub arguments: &'static [Expr],
}

spanned!(Return);
#[derive(Debug, Clone, Copy)]
pub struct Return {
    pub span: Span,
    pub expr: Option<Expr>,
}

spanned!(If);
#[derive(Debug, Clone, Copy)]
pub struct If {
    pub span: Span,
    pub condition: Expr,
    pub body: Block,
}

spanned!(While);
#[derive(Debug, Clone, Copy)]
pub struct While {
    pub span: Span,
    pub condition: Expr,
    pub body: Block,
}
