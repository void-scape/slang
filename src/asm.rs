use crate::ir::{Arg, BinOp, Data, Func, Ir, UnaryOp};
use std::collections::HashMap;

#[derive(Default)]
struct Mem {
    frames: Vec<(HashMap<usize, usize>, usize)>,
    data_map: HashMap<Data, &'static str>,
    data: u64,
}

impl Mem {
    fn push_frame(&mut self) {
        self.frames.push((HashMap::default(), 8));
    }

    fn offset(&mut self, var: usize) -> usize {
        let last = self.frames.len() - 1;
        let (map, offset) = &mut self.frames[last];
        *map.entry(var).or_insert_with(|| {
            let of = *offset;
            *offset += 8;
            of
        })
    }

    fn data(&mut self, data: Data) -> &'static str {
        self.data_map.entry(data).or_insert_with(|| {
            let label = format!("d{}", self.data).leak();
            self.data += 1;
            label
        })
    }

    fn pop_frame(&mut self) -> usize {
        let (_, bytes) = self.frames.pop().unwrap();
        bytes
    }
}

fn load(asm: &mut String, mem: &mut Mem, funcs: &[Func], arg: Arg, reg: u8) {
    match arg {
        Arg::Invalid => {
            panic!("expected valid argument");
        }
        Arg::Var(v) => {
            let src = mem.offset(v);
            asm.push_str(&format!("\tldr x{reg}, [x29, -{src}]\n"));
        }
        Arg::Lit(lit) => {
            if lit.bit_width() <= 12 {
                asm.push_str(&format!("\tmov x{reg}, {lit}\n"));
            } else {
                asm.push_str(&format!("\tldr x{reg}, ={lit}\n"));
            }
        }
        Arg::Data(data) => {
            let label = mem.data(data);
            asm.push_str(&format!("\tadrp x{reg}, {label}@PAGE\n"));
            asm.push_str(&format!("\tadd x{reg}, x{reg}, {label}@PAGEOFF\n"));
        }
        Arg::CallReturn(ident) => {
            let func = funcs
                .iter()
                .find(|f| f.ident == ident)
                .expect("valid function");
            assert_eq!(func.returns.len(), 1);
            asm.push_str(&format!("\tmov x{reg}, x0\n"));
        }
    }
}

fn store(asm: &mut String, mem: &mut Mem, dst: Arg, reg: u8) {
    let dst = match dst {
        Arg::Var(var) => mem.offset(var),
        a => panic!("cannot write to {a:?}"),
    };
    asm.push_str(&format!("\tstr x{reg}, [x29, -{dst}]\n"));
}

fn asm_func(asm: &mut String, mem: &mut Mem, funcs: &[Func], func: &Func) {
    mem.push_frame();
    let mut cleanup_points = Vec::new();
    asm.push_str(&format!("_{}:\n", func.ident));
    let allocate_stack = asm.len();
    // store all of the register arguments onto the stack
    for (i, arg) in func.params.iter().enumerate() {
        if i < 8 {
            store(asm, mem, *arg, i as u8);
        } else {
            asm.push_str(&format!("\tldr x0, [x29, {}]\n", 8 + (i - 7) * 8));
            store(asm, mem, *arg, 0);
        }
    }
    if func.variadic {
        panic!("variadics are fake lol");
    }
    for op in func.body.iter() {
        match op {
            Ir::Store { dst, src } => {
                let reg = 8;
                load(asm, mem, funcs, *src, reg);
                store(asm, mem, *dst, reg);
            }
            Ir::Unary { dst, src, una } => match una {
                UnaryOp::Not => {
                    let scratch = 8;
                    load(asm, mem, funcs, *src, scratch);
                    asm.push_str(&format!("\tmvn x{scratch}, x{scratch}\n"));
                    store(asm, mem, *dst, scratch);
                }
            },
            Ir::Bin { dst, lhs, rhs, bin } => {
                load(asm, mem, funcs, *lhs, 9);
                load(asm, mem, funcs, *rhs, 10);
                match bin {
                    BinOp::Add
                    | BinOp::Sub
                    | BinOp::Mul
                    | BinOp::Div
                    | BinOp::BitAnd
                    | BinOp::BitOr
                    | BinOp::Shr
                    | BinOp::Shl
                    | BinOp::Xor => {
                        let op = match bin {
                            BinOp::Add => "add",
                            BinOp::Sub => "sub",
                            BinOp::Mul => "mul",
                            BinOp::Div => "udiv",
                            BinOp::BitAnd => "and",
                            BinOp::BitOr => "orr",
                            BinOp::Xor => "eor",
                            BinOp::Shr => "lsr",
                            BinOp::Shl => "lsl",
                            _ => unreachable!(),
                        };
                        let reg = 8;
                        asm.push_str(&format!("\t{op} x{reg}, x9, x10\n"));
                        store(asm, mem, *dst, reg);
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Gt | BinOp::Lt | BinOp::Ge | BinOp::Le => {
                        let cond = match bin {
                            BinOp::Eq => "eq",
                            BinOp::Ne => "ne",
                            BinOp::Gt => "gt",
                            BinOp::Ge => "ge",
                            BinOp::Lt => "lt",
                            BinOp::Le => "le",
                            _ => unreachable!(),
                        };
                        let reg = 8;
                        asm.push_str("\tcmp x9, x10\n");
                        asm.push_str(&format!("\tcset x{reg}, {cond}\n"));
                        store(asm, mem, *dst, reg);
                    }
                    BinOp::Mod => {
                        let reg = 8;
                        asm.push_str(&format!("\tudiv x{reg}, x9, x10\n"));
                        asm.push_str(&format!("\tmsub x{reg}, x{reg}, x10, x9\n"));
                        store(asm, mem, *dst, reg);
                    }
                    BinOp::And | BinOp::Or => unreachable!(),
                }
            }
            Ir::Label { label } => {
                asm.push_str(&format!("l{label}:\n"));
            }
            Ir::Jump { label } => {
                asm.push_str(&format!("\tb l{label}\n"));
            }
            Ir::JumpZero { label, arg } => {
                load(asm, mem, funcs, *arg, 8);
                asm.push_str(&format!("\tcbz x8, l{label}\n"));
            }
            Ir::JumpNotZero { label, arg } => {
                load(asm, mem, funcs, *arg, 8);
                asm.push_str(&format!("\tcbnz x8, l{label}\n"));
            }
            Ir::Call { symbol, args } => {
                let func = funcs
                    .iter()
                    .find(|f| f.ident == *symbol)
                    .expect("valid function");
                if args.len() > func.params.len() {
                    assert!(func.variadic);
                } else {
                    assert_eq!(args.len(), func.params.len());
                }

                let allocate_stack = asm.len();
                let mut allocated: usize = 0;
                for (i, arg) in args.iter().enumerate() {
                    if i < func.params.len() && i < 8 {
                        load(asm, mem, funcs, *arg, i as u8);
                    } else {
                        allocated += 1;
                        load(asm, mem, funcs, *arg, 8);
                        asm.push_str(&format!(
                            "\tstr x8, [sp, {}]\n",
                            (i - func.params.len().min(8)) * 8
                        ));
                    }
                }
                if allocated > 0 {
                    let arg_stack = (allocated * 8).div_ceil(16) * 16;
                    asm.insert_str(allocate_stack, &format!("\tsub sp, sp, {arg_stack}\n"));
                    asm.push_str(&format!("\tbl _{symbol}\n"));
                    asm.push_str(&format!("\tadd sp, sp, {arg_stack}\n"));
                } else {
                    asm.push_str(&format!("\tbl _{symbol}\n"));
                }
            }
            Ir::Return { arg } => match arg {
                Some(arg) => {
                    load(asm, mem, funcs, *arg, 0);
                    cleanup_points.push(asm.len());
                    asm.push_str("\tret\n");
                }
                None => {
                    cleanup_points.push(asm.len());
                    asm.push_str("\tret\n");
                }
            },
        }
    }
    let bytes = mem.pop_frame();
    let stack = bytes.div_ceil(16) * 16;
    let cleanup_stack = format!(
        "\tldp x29, x30, [sp, {stack}]\n\
        \tadd sp, sp, {}\n",
        stack + 16,
    );
    for index in cleanup_points.iter().rev() {
        asm.insert_str(*index, &cleanup_stack);
    }
    if func
        .body
        .last()
        .is_none_or(|t| !matches!(t, Ir::Return { .. }))
    {
        asm.push_str(&cleanup_stack);
        asm.push_str("\tmov x0, 0\n");
        asm.push_str("\tret\n");
    }
    asm.insert_str(
        allocate_stack,
        &format!(
            "\tsub sp, sp, {}\n\
            \tstp x29, x30, [sp, {stack}]\n\
            \tadd x29, sp, {stack}\n",
            stack + 16,
        ),
    );
}

pub fn asm(funcs: &[Func]) -> String {
    let mut asm = String::from(".text\n");
    asm.push_str(".global _start\n");
    asm.push_str("_start:\n");
    asm.push_str("\tbl _main\n");
    asm.push_str("\tmov x16, 1\n");
    asm.push_str("\tsvc 0x80\n");
    let mut mem = Mem::default();
    for func in funcs.iter() {
        // TODO: attributes
        if func.ident == "printf" || func.ident == "exit" {
            continue;
        }
        asm_func(&mut asm, &mut mem, funcs, func);
        mem.frames.clear();
    }
    asm.push_str(".data\n");
    for (data, label) in mem.data_map.iter() {
        match data {
            Data::Str(str) => {
                asm.push_str(&format!("\t{label}: .asciz \"{str}\"\n"));
            }
        }
    }

    asm
}
