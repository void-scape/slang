#[derive(Debug)]
pub enum Ir {
    Store {
        dst: Arg,
        src: Arg,
    },
    Unary {
        dst: Arg,
        src: Arg,
        una: UnaryOp,
    },
    Bin {
        dst: Arg,
        lhs: Arg,
        rhs: Arg,
        bin: BinOp,
    },
    Label {
        label: usize,
    },
    Jump {
        label: usize,
    },
    JumpZero {
        label: usize,
        arg: Arg,
    },
    JumpNotZero {
        label: usize,
        arg: Arg,
    },
    Call {
        symbol: &'static str,
        // TODO: Store in an arena and make this a static reference.
        args: Vec<Arg>,
    },
    Return {
        arg: Option<Arg>,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum Arg {
    #[allow(unused)]
    Invalid,
    Var(usize),
    Lit(u64),
    Data(Data),
    CallReturn(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Data {
    Str(&'static str),
}

#[derive(Debug)]
pub struct Func {
    pub ident: &'static str,
    pub params: Vec<Arg>,
    pub variadic: bool,
    pub returns: Vec<()>,
    pub body: Vec<Ir>,
}
