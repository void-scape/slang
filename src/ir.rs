use crate::{
    ast::{
        Ast, BinOp, BinOpKind, Block, Call, Expr, Func, Ident, If, Layout, LiteralKind, Return,
        Spanned, TypeKind, UnaryOp, UnaryOpKind, While,
    },
    tokenize::Span,
};
use std::{collections::HashMap, io::IsTerminal, panic::Location};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    kind: ErrorKind,
    span: Span,
    location: &'static Location<'static>,
}

trait SpannedError {
    fn error(&self, kind: ErrorKind) -> Error;
}

impl<T: Spanned> SpannedError for T {
    #[track_caller]
    fn error(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            span: self.span(),
            location: Location::caller(),
        }
    }
}

impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ErrorKind::GlobalScope => {
                f.write_str("Expression can not be defined in the global scope")
            }
            ErrorKind::UnknownType => f.write_str("Can not infer the type"),
            ErrorKind::Undefined { ident } => f.write_str(&format!("`{ident}` is undefined")),
            ErrorKind::ExpectedRValue => f.write_str("This expression does not produce a value"),
            ErrorKind::ExpectedLValue => f.write_str("This expression can not be assigned"),
            ErrorKind::ReturnMismatch { expected, got } => f.write_str(&format!(
                "Expected function to return {expected} values, not {got}"
            )),
            ErrorKind::ArgumentMismatch { expected, got } => {
                f.write_str(&format!("Expected {expected} arguments, got {got}"))
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ErrorKind {
    GlobalScope,
    UnknownType,
    Undefined { ident: &'static str },
    ExpectedRValue,
    ExpectedLValue,
    ReturnMismatch { expected: usize, got: usize },
    ArgumentMismatch { expected: usize, got: usize },
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

#[derive(Debug)]
pub enum Ir {
    Store {
        dst: Arg,
        src: Arg,
    },
    Unary {
        dst: Arg,
        src: Arg,
        una: UnaryOpKind,
    },
    Bin {
        dst: Arg,
        lhs: Arg,
        rhs: Arg,
        bin: IrBinOpKind,
    },
    Label {
        label: &'static str,
    },
    Jump {
        label: &'static str,
    },
    JumpZero {
        label: &'static str,
        arg: Arg,
    },
    JumpNotZero {
        label: &'static str,
        arg: Arg,
    },
    Allocate {
        size: usize,
        slots: Vec<StackSlot>,
    },
    LoadArguments {
        arguments: Vec<(Arg, Layout)>,
    },
    Call {
        symbol: &'static str,
        named: usize,
        arguments: Vec<(Arg, Layout)>,
        results: Vec<(Arg, Layout)>,
    },
    Return {
        results: Vec<(Arg, Layout)>,
        deallocate: usize,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum Arg {
    Var(usize),
    Lit(u64),
    Data(Data),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Data {
    Str(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrBinOpKind {
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
    BitAnd,
    BitOr,
    Xor,
    Shr,
    Shl,
}

impl IrBinOpKind {
    fn expect_from_ast_bin_op_kind(kind: BinOpKind) -> Self {
        match kind {
            BinOpKind::Add => Self::Add,
            BinOpKind::Sub => Self::Sub,
            BinOpKind::Mul => Self::Mul,
            BinOpKind::Div => Self::Div,
            BinOpKind::Mod => Self::Mod,
            BinOpKind::Eq => Self::Eq,
            BinOpKind::Ne => Self::Ne,
            BinOpKind::Gt => Self::Gt,
            BinOpKind::Ge => Self::Ge,
            BinOpKind::Lt => Self::Lt,
            BinOpKind::Le => Self::Le,
            BinOpKind::BitAnd => Self::BitAnd,
            BinOpKind::BitOr => Self::BitOr,
            BinOpKind::Xor => Self::Xor,
            BinOpKind::Shr => Self::Shr,
            BinOpKind::Shl => Self::Shl,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StackSlot {
    pub aligned_offset: usize,
    pub layout: Layout,
}

#[derive(Default)]
struct MemoryMap {
    data: HashMap<Data, Arg>,
    map: HashMap<&'static str, usize>,
    vars: Vec<StackSlot>,
}

impl MemoryMap {
    fn allocate_var(&mut self, ident: &'static str, layout: Layout) -> Arg {
        let arg = self.allocate(layout);
        let Arg::Var(index) = arg else { unreachable!() };
        assert!(self.map.insert(ident, index).is_none());
        arg
    }

    fn allocate(&mut self, layout: Layout) -> Arg {
        let len = self.vars.len();
        match self.vars.last() {
            Some(last) => {
                let size = last.layout.size;
                self.vars.push(StackSlot {
                    aligned_offset: (last.aligned_offset + size).next_multiple_of(layout.align),
                    layout,
                });
            }
            None => {
                self.vars.push(StackSlot {
                    aligned_offset: 0,
                    layout,
                });
            }
        }
        Arg::Var(len)
    }

    fn var(&self, ident: &Ident) -> Result<Arg> {
        self.map
            .get(ident.value)
            .map(|i| Arg::Var(*i))
            .ok_or(ident.error(ErrorKind::Undefined { ident: ident.value }))
    }

    fn data(&mut self, data: Data) -> Arg {
        *self.data.entry(data).or_insert(Arg::Data(data))
    }

    fn size(&self) -> usize {
        self.vars
            .last()
            .map(|s| s.aligned_offset + s.layout.size)
            .unwrap_or(0)
    }
}

#[derive(Default)]
struct Labels {
    prefix: &'static str,
    unique: usize,
}

impl Labels {
    fn prelude(&self) -> &'static str {
        format!("{}_prelude", self.prefix).leak()
    }

    fn body(&self) -> &'static str {
        format!("{}_body", self.prefix).leak()
    }

    fn label(&mut self, name: &'static str) -> &'static str {
        let str = format!("{}_{}{}", self.prefix, name, self.unique);
        self.unique += 1;
        str.leak()
    }

    fn epilogue(&self) -> &'static str {
        format!("{}_epilogue", self.prefix).leak()
    }
}

fn whilee(
    memory: &mut MemoryMap,
    labels: &mut Labels,
    ir: &mut Vec<Ir>,
    funcs: &Funcs,
    return_args: &[(Arg, Layout)],
    w: &While,
) -> Result<()> {
    // TODO: verbosity flag
    let condition = labels.label("while_condition");
    let body = labels.label("while_body");
    let exit = labels.label("while_exit");

    ir.push(Ir::Label { label: condition });
    let arg = rvalue_expr(memory, labels, ir, funcs, return_args, &w.condition)?;
    ir.push(Ir::JumpZero { label: exit, arg });
    ir.push(Ir::Label { label: body });
    block(memory, labels, ir, funcs, return_args, &w.body)?;
    ir.push(Ir::Jump { label: condition });
    ir.push(Ir::Label { label: exit });

    Ok(())
}

fn iff(
    memory: &mut MemoryMap,
    labels: &mut Labels,
    ir: &mut Vec<Ir>,
    funcs: &Funcs,
    return_args: &[(Arg, Layout)],
    i: &If,
) -> Result<()> {
    // TODO: verbosity flag
    let condition = labels.label("if_condition");
    let body = labels.label("if_body");
    let skip = labels.label("if_skip");

    ir.push(Ir::Label { label: condition });
    let arg = rvalue_expr(memory, labels, ir, funcs, return_args, &i.condition)?;
    ir.push(Ir::JumpZero { label: skip, arg });
    ir.push(Ir::Label { label: body });
    block(memory, labels, ir, funcs, return_args, &i.body)?;
    ir.push(Ir::Label { label: skip });

    Ok(())
}

fn ret(
    memory: &mut MemoryMap,
    labels: &mut Labels,
    ir: &mut Vec<Ir>,
    funcs: &Funcs,
    return_args: &[(Arg, Layout)],
    r: &Return,
) -> Result<()> {
    match r.expr {
        Some(e) => {
            if return_args.len() != 1 {
                return Err(r.error(ErrorKind::ReturnMismatch {
                    expected: return_args.len(),
                    got: 1,
                }));
            }
            let arg = rvalue_expr(memory, labels, ir, funcs, return_args, &e)?;
            ir.push(Ir::Store {
                dst: return_args[0].0,
                src: arg,
            });
        }
        None => {
            if !return_args.is_empty() {
                return Err(r.error(ErrorKind::ReturnMismatch {
                    expected: return_args.len(),
                    got: 0,
                }));
            }
        }
    }
    ir.push(Ir::Jump {
        label: labels.epilogue(),
    });
    Ok(())
}

fn bin_op(
    memory: &mut MemoryMap,
    labels: &mut Labels,
    ir: &mut Vec<Ir>,
    funcs: &Funcs,
    return_args: &[(Arg, Layout)],
    bo: &BinOp,
) -> Result<Arg> {
    match bo.kind {
        BinOpKind::And => {
            let dst = memory.allocate(TypeKind::U64.layout());
            let skip = labels.label("and_skip");
            let lhs = rvalue_expr(memory, labels, ir, funcs, return_args, bo.lhs)?;
            ir.push(Ir::Store { dst, src: lhs });
            ir.push(Ir::JumpZero {
                label: skip,
                arg: lhs,
            });
            let rhs = rvalue_expr(memory, labels, ir, funcs, return_args, bo.rhs)?;
            ir.push(Ir::Store { dst, src: rhs });
            ir.push(Ir::Label { label: skip });
            Ok(dst)
        }
        BinOpKind::Or => {
            let dst = memory.allocate(TypeKind::U64.layout());
            let skip = labels.label("or_skip");
            let lhs = rvalue_expr(memory, labels, ir, funcs, return_args, bo.lhs)?;
            ir.push(Ir::Store { dst, src: lhs });
            ir.push(Ir::JumpNotZero {
                label: skip,
                arg: lhs,
            });
            let rhs = rvalue_expr(memory, labels, ir, funcs, return_args, bo.rhs)?;
            ir.push(Ir::Store { dst, src: rhs });
            ir.push(Ir::Label { label: skip });
            Ok(dst)
        }
        BinOpKind::Assign
        | BinOpKind::AddAssign
        | BinOpKind::SubAssign
        | BinOpKind::MulAssign
        | BinOpKind::DivAssign
        | BinOpKind::ModAssign
        | BinOpKind::BitAndAssign
        | BinOpKind::BitOrAssign
        | BinOpKind::XorAssign
        | BinOpKind::ShlAssign
        | BinOpKind::ShrAssign => {
            let lhs = lvalue_expr(memory, labels, ir, funcs, return_args, bo.lhs)?;
            let rhs = rvalue_expr(memory, labels, ir, funcs, return_args, bo.rhs)?;
            if bo.kind == BinOpKind::Assign {
                ir.push(Ir::Store { dst: lhs, src: rhs });
                Ok(lhs)
            } else {
                let bin = match bo.kind {
                    BinOpKind::AddAssign => IrBinOpKind::Add,
                    BinOpKind::SubAssign => IrBinOpKind::Sub,
                    BinOpKind::MulAssign => IrBinOpKind::Mul,
                    BinOpKind::DivAssign => IrBinOpKind::Div,
                    BinOpKind::ModAssign => IrBinOpKind::Mod,
                    BinOpKind::BitAndAssign => IrBinOpKind::BitAnd,
                    BinOpKind::BitOrAssign => IrBinOpKind::BitOr,
                    BinOpKind::XorAssign => IrBinOpKind::Xor,
                    BinOpKind::ShlAssign => IrBinOpKind::Shl,
                    BinOpKind::ShrAssign => IrBinOpKind::Shr,
                    _ => unreachable!(),
                };
                ir.push(Ir::Bin {
                    dst: lhs,
                    lhs,
                    rhs,
                    bin,
                });
                Ok(lhs)
            }
        }
        _ => {
            let lhs = rvalue_expr(memory, labels, ir, funcs, return_args, bo.lhs)?;
            let rhs = rvalue_expr(memory, labels, ir, funcs, return_args, bo.rhs)?;
            let dst = memory.allocate(TypeKind::U64.layout());
            ir.push(Ir::Bin {
                dst,
                lhs,
                rhs,
                bin: IrBinOpKind::expect_from_ast_bin_op_kind(bo.kind),
            });
            Ok(dst)
        }
    }
}

fn unary_op(
    memory: &mut MemoryMap,
    labels: &mut Labels,
    ir: &mut Vec<Ir>,
    funcs: &Funcs,
    return_args: &[(Arg, Layout)],
    uo: &UnaryOp,
) -> Result<Arg> {
    match uo.kind {
        UnaryOpKind::Not => {
            let dst = memory.allocate(TypeKind::U64.layout());
            let src = rvalue_expr(memory, labels, ir, funcs, return_args, uo.expr)?;
            ir.push(Ir::Unary {
                dst,
                src,
                una: uo.kind,
            });
            Ok(dst)
        }
    }
}

fn call(
    memory: &mut MemoryMap,
    labels: &mut Labels,
    ir: &mut Vec<Ir>,
    funcs: &Funcs,
    return_args: &[(Arg, Layout)],
    c: &Call,
    results: Vec<(Arg, Layout)>,
) -> Result<()> {
    let func = funcs.func(&c.ident)?;
    if func.arguments.len() != c.arguments.len()
        && (c.arguments.len() < func.arguments.len() || !func.variadic)
    {
        return Err(c.error(ErrorKind::ArgumentMismatch {
            expected: func.arguments.len(),
            got: c.arguments.len(),
        }));
    }
    let mut arguments = Vec::new();
    for (arg, decl) in c.arguments.iter().zip(func.arguments) {
        let ty = decl.ty.ok_or(arg.error(ErrorKind::UnknownType))?;
        arguments.push((
            rvalue_expr(memory, labels, ir, funcs, return_args, arg)?,
            ty.kind.layout(),
        ));
    }
    if func.variadic {
        for arg in c.arguments.iter().skip(func.arguments.len()) {
            let ty = TypeKind::U64;
            arguments.push((
                rvalue_expr(memory, labels, ir, funcs, return_args, arg)?,
                ty.layout(),
            ));
        }
    }
    let symbol = match c.ident.value {
        "printf" => "_printf",
        "exit" => "_exit",
        symbol => symbol,
    };
    ir.push(Ir::Call {
        symbol,
        named: func.arguments.len(),
        arguments,
        results,
    });
    Ok(())
}

fn call_and_store(
    memory: &mut MemoryMap,
    labels: &mut Labels,
    ir: &mut Vec<Ir>,
    funcs: &Funcs,
    return_args: &[(Arg, Layout)],
    c: &Call,
) -> Result<Arg> {
    let dst = memory.allocate(TypeKind::U64.layout());
    call(
        memory,
        labels,
        ir,
        funcs,
        return_args,
        c,
        vec![(dst, TypeKind::U64.layout())],
    )?;
    Ok(dst)
}

fn call_and_ignore(
    memory: &mut MemoryMap,
    labels: &mut Labels,
    ir: &mut Vec<Ir>,
    funcs: &Funcs,
    return_args: &[(Arg, Layout)],
    c: &Call,
) -> Result<()> {
    call(memory, labels, ir, funcs, return_args, c, Vec::new())
}

fn rvalue_expr(
    memory: &mut MemoryMap,
    labels: &mut Labels,
    ir: &mut Vec<Ir>,
    funcs: &Funcs,
    return_args: &[(Arg, Layout)],
    e: &Expr,
) -> Result<Arg> {
    match e {
        Expr::Ident(ident) => memory.var(ident),
        Expr::Literal(literal) => Ok(match literal.kind {
            LiteralKind::Integer(lit) => Arg::Lit(lit),
            LiteralKind::Str(str) => memory.data(Data::Str(str)),
        }),
        Expr::BinOp(bo) => bin_op(memory, labels, ir, funcs, return_args, bo),
        Expr::UnaryOp(uo) => unary_op(memory, labels, ir, funcs, return_args, uo),
        Expr::Declaration(decl) => Err(decl.error(ErrorKind::ExpectedRValue)),
        Expr::Call(c) => call_and_store(memory, labels, ir, funcs, return_args, c),
    }
}

fn lvalue_expr(
    memory: &mut MemoryMap,
    _labels: &mut Labels,
    _ir: &mut Vec<Ir>,
    _funcs: &Funcs,
    _return_args: &[(Arg, Layout)],
    e: &Expr,
) -> Result<Arg> {
    match e {
        Expr::Ident(ident) => memory.var(ident),
        Expr::Literal(l) => Err(l.error(ErrorKind::ExpectedLValue)),
        Expr::BinOp(bo) => Err(bo.error(ErrorKind::ExpectedLValue)),
        // TODO: deref
        Expr::UnaryOp(uo) => Err(uo.error(ErrorKind::ExpectedLValue)),
        Expr::Call(c) => Err(c.error(ErrorKind::ExpectedLValue)),
        Expr::Declaration(d) => Err(d.error(ErrorKind::ExpectedLValue)),
    }
}

fn expr_statement(
    memory: &mut MemoryMap,
    labels: &mut Labels,
    ir: &mut Vec<Ir>,
    funcs: &Funcs,
    return_args: &[(Arg, Layout)],
    e: &Expr,
) -> Result<()> {
    match e {
        Expr::Ident(_) | Expr::Literal(_) => {
            // noops
        }
        Expr::BinOp(_) | Expr::UnaryOp(_) => {
            rvalue_expr(memory, labels, ir, funcs, return_args, e)?;
        }
        Expr::Call(c) => {
            call_and_ignore(memory, labels, ir, funcs, return_args, c)?;
        }
        Expr::Declaration(decl) => {
            let ty = decl.ty.ok_or(decl.ident.error(ErrorKind::UnknownType))?;
            // TODO: Zero?
            let arg = memory.allocate_var(decl.ident.value, ty.layout);
            if let Some(rhs) = decl.rhs {
                let result = rvalue_expr(memory, labels, ir, funcs, return_args, rhs)?;
                ir.push(Ir::Store {
                    dst: arg,
                    src: result,
                });
            }
        }
    }
    Ok(())
}

fn block(
    memory: &mut MemoryMap,
    labels: &mut Labels,
    ir: &mut Vec<Ir>,
    funcs: &Funcs,
    return_args: &[(Arg, Layout)],
    b: &Block,
) -> Result<()> {
    for ast in b.statements.iter() {
        match ast {
            Ast::Func(_) => unimplemented!("local functions"),
            Ast::Block(b) => block(memory, labels, ir, funcs, return_args, b)?,
            Ast::Expr(e) => expr_statement(memory, labels, ir, funcs, return_args, e)?,
            Ast::Return(r) => ret(memory, labels, ir, funcs, return_args, r)?,
            Ast::If(i) => iff(memory, labels, ir, funcs, return_args, i)?,
            Ast::While(w) => whilee(memory, labels, ir, funcs, return_args, w)?,
        }
    }
    Ok(())
}

fn func(labels: &mut Labels, ir: &mut Vec<Ir>, funcs: &Funcs, func: &Func) -> Result<()> {
    if func.ident.value == "printf" || func.ident.value == "exit" {
        return Ok(());
    }

    labels.prefix = func.ident.value;
    ir.push(Ir::Label {
        label: labels.prefix,
    });
    ir.push(Ir::Label {
        label: labels.prelude(),
    });
    let allocate_stack = ir.len();
    let mut memory = MemoryMap::default();
    assert!(
        func.arguments
            .iter()
            .all(|a| a.ty.is_some_and(|t| t.kind == TypeKind::U64))
    );
    // allocate the function arguments
    let mut arguments = Vec::with_capacity(func.arguments.len());
    for arg in func.arguments.iter() {
        let ty = arg.ty.ok_or(arg.error(ErrorKind::UnknownType))?;
        arguments.push((memory.allocate_var(arg.ident.value, ty.layout), ty.layout));
    }
    ir.push(Ir::LoadArguments { arguments });
    // allocate the function return values
    let mut return_args = Vec::with_capacity(func.returns.len());
    for ret in func.returns.iter() {
        let ty = ret.ty.ok_or(ret.error(ErrorKind::UnknownType))?;
        return_args.push((memory.allocate(ty.layout), ty.layout));
    }
    ir.push(Ir::Label {
        label: labels.body(),
    });
    block(&mut memory, labels, ir, funcs, &return_args, &func.body)?;
    // the function has returned somewhere and now the arguments must be passed
    // out of the function body
    ir.push(Ir::Label {
        label: labels.epilogue(),
    });
    let stack_size = memory.size();
    ir.insert(
        allocate_stack,
        Ir::Allocate {
            size: stack_size,
            slots: memory.vars.clone(),
        },
    );
    ir.push(Ir::Return {
        results: return_args,
        deallocate: stack_size,
    });
    Ok(())
}

struct Funcs<'a>(HashMap<&'static str, &'a Func>);
impl Funcs<'_> {
    #[track_caller]
    fn func(&self, ident: &Ident) -> Result<&Func> {
        self.0
            .get(ident.value)
            .copied()
            .ok_or(ident.error(ErrorKind::Undefined { ident: ident.value }))
    }
}

pub fn ir(tree: &[Ast]) -> Result<Vec<Ir>> {
    let funcs = Funcs(
        tree.iter()
            .flat_map(|a| match a {
                Ast::Func(f) => Some((f.ident.value, f)),
                _ => None,
            })
            .collect::<HashMap<_, _>>(),
    );
    let mut labels = Labels::default();
    let mut ir = Vec::new();
    for node in tree.iter() {
        match node {
            Ast::Func(f) => func(&mut labels, &mut ir, &funcs, f)?,
            ast => return Err(ast.error(ErrorKind::GlobalScope)),
        }
    }
    Ok(ir)
}
