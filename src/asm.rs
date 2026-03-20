use crate::{
    ast::UnaryOpKind,
    ir::{Arg, Data, Ir, IrBinOpKind, StackSlot},
};
use std::collections::HashMap;

#[derive(Default)]
struct DataSection {
    map: HashMap<Data, &'static str>,
    unique: usize,
}

impl DataSection {
    fn data(&mut self, data: Data) -> &'static str {
        self.map.entry(data).or_insert_with(|| {
            let label = format!("d{}", self.unique);
            self.unique += 1;
            label.leak()
        })
    }
}

fn load(asm: &mut String, data: &mut DataSection, stack: &[StackSlot], dst: u8, src: Arg) {
    match src {
        Arg::Var(src) => {
            let src_offset = stack[src].aligned_offset + 8;
            asm.push_str(&format!("\tldr x{dst}, [x29, -{src_offset}]\n"));
        }
        Arg::Lit(lit) => {
            if lit.bit_width() <= 12 {
                asm.push_str(&format!("\tmov x{dst}, {lit}\n"));
            } else {
                asm.push_str(&format!("\tldr x{dst}, ={lit}\n"));
            }
        }
        Arg::Data(d) => {
            let label = data.data(d);
            asm.push_str(&format!("\tadrp x{dst}, {label}@PAGE\n"));
            asm.push_str(&format!("\tadd x{dst}, x{dst}, {label}@PAGEOFF\n"));
        }
    }
}

fn store_reg(asm: &mut String, stack: &[StackSlot], dst: Arg, src: u8) {
    match dst {
        Arg::Var(dst) => {
            let dst_offset = stack[dst].aligned_offset + 8;
            asm.push_str(&format!("\tstr x{src}, [x29, -{dst_offset}]\n"));
        }
        dst => panic!("cannot store into {dst:?}"),
    }
}

fn store(asm: &mut String, data: &mut DataSection, stack: &[StackSlot], dst: Arg, src: Arg) {
    match dst {
        Arg::Var(dst) => {
            let scratch = 9;
            load(asm, data, stack, scratch, src);
            let dst_offset = stack[dst].aligned_offset + 8;
            asm.push_str(&format!("\tstr x{scratch}, [x29, -{dst_offset}]\n"));
        }
        dst => panic!("cannot store into {dst:?}"),
    }
}

pub fn asm(ir: &[Ir]) -> String {
    let mut asm = String::new();
    asm.push_str(".global _start\n");
    asm.push_str("_start:\n");
    asm.push_str("\tbl main\n");
    asm.push_str("\tb _exit\n");
    let mut data = DataSection::default();
    let mut stack = None;
    let mut callee_stack_offset = 0;
    // temp r9..r15
    let sa = 9;
    let sb = 10;
    let sc = 11;
    for op in ir.iter() {
        match op {
            Ir::Store { dst, src } => {
                store(&mut asm, &mut data, stack.unwrap(), *dst, *src);
            }
            Ir::Unary { dst, src, una } => match una {
                UnaryOpKind::Not => {
                    load(&mut asm, &mut data, stack.unwrap(), sa, *src);
                    asm.push_str(&format!("\tmvn x{sa}, x{sa}\n"));
                    store_reg(&mut asm, stack.unwrap(), *dst, sa);
                }
            },
            Ir::Bin { dst, lhs, rhs, bin } => {
                load(&mut asm, &mut data, stack.unwrap(), sb, *lhs);
                load(&mut asm, &mut data, stack.unwrap(), sc, *rhs);
                match bin {
                    IrBinOpKind::Add
                    | IrBinOpKind::Sub
                    | IrBinOpKind::Mul
                    | IrBinOpKind::Div
                    | IrBinOpKind::BitAnd
                    | IrBinOpKind::BitOr
                    | IrBinOpKind::Shr
                    | IrBinOpKind::Shl
                    | IrBinOpKind::Xor => {
                        let op = match bin {
                            IrBinOpKind::Add => "add",
                            IrBinOpKind::Sub => "sub",
                            IrBinOpKind::Mul => "mul",
                            IrBinOpKind::Div => "udiv",
                            IrBinOpKind::BitAnd => "and",
                            IrBinOpKind::BitOr => "orr",
                            IrBinOpKind::Xor => "eor",
                            IrBinOpKind::Shr => "lsr",
                            IrBinOpKind::Shl => "lsl",
                            _ => unreachable!(),
                        };
                        asm.push_str(&format!("\t{op} x{sa}, x{sb}, x{sc}\n"));
                        store_reg(&mut asm, stack.unwrap(), *dst, sa);
                    }
                    IrBinOpKind::Eq
                    | IrBinOpKind::Ne
                    | IrBinOpKind::Gt
                    | IrBinOpKind::Lt
                    | IrBinOpKind::Ge
                    | IrBinOpKind::Le => {
                        let cond = match bin {
                            IrBinOpKind::Eq => "eq",
                            IrBinOpKind::Ne => "ne",
                            IrBinOpKind::Gt => "gt",
                            IrBinOpKind::Ge => "ge",
                            IrBinOpKind::Lt => "lt",
                            IrBinOpKind::Le => "le",
                            _ => unreachable!(),
                        };
                        asm.push_str(&format!("\tcmp x{sb}, x{sc}\n"));
                        asm.push_str(&format!("\tcset x{sa}, {cond}\n"));
                        store_reg(&mut asm, stack.unwrap(), *dst, sa);
                    }
                    IrBinOpKind::Mod => {
                        asm.push_str(&format!("\tudiv x{sa}, x{sb}, x{sc}\n"));
                        asm.push_str(&format!("\tmsub x{sa}, x{sa}, x{sc}, x{sb}\n"));
                        store_reg(&mut asm, stack.unwrap(), *dst, sa);
                    }
                }
            }
            Ir::Label { label } => {
                asm.push_str(&format!("{label}:\n"));
            }
            Ir::Jump { label } => {
                asm.push_str(&format!("\tb {label}\n"));
            }
            Ir::JumpZero { label, arg } => {
                load(&mut asm, &mut data, stack.unwrap(), sa, *arg);
                asm.push_str(&format!("\tcbz x{sa}, {label}\n"));
            }
            Ir::JumpNotZero { label, arg } => {
                load(&mut asm, &mut data, stack.unwrap(), sa, *arg);
                asm.push_str(&format!("\tcbnz x{sa}, {label}\n"));
            }
            Ir::Allocate { size, slots } => {
                stack = Some(slots);
                let stack_size = size.next_multiple_of(16);
                callee_stack_offset = stack_size + 16;
                asm.push_str(&format!("\tsub sp, sp, {}\n", callee_stack_offset));
                asm.push_str(&format!("\tstp x29, x30, [sp, {stack_size}]\n"));
                asm.push_str(&format!("\tadd x29, sp, {stack_size}\n"));
            }
            Ir::Call {
                symbol,
                named,
                arguments,
                results,
            } => {
                let stack = stack.unwrap();
                let allocate_stack = asm.len();

                // The AAPCS64 calling convection as defined here:
                // https://student.cs.uwaterloo.ca/~cs452/docs/rpi4b/aapcs64.pdf

                // Initialization
                // A.1: The Next General-purpose Register Number (NGRN) is set
                // to zero.
                let mut ngrn: usize = 0;
                // A.2: The Next SIMD and Floating-point Register Number (NSRN)
                // is set to zero.
                let _nsrn = 0;
                // A.3: The Next Scalable Predicate Register Number (NPRN) is
                // set to zero
                let _nprn = 0;
                // A.4: The next stacked argument address (NSAA) is set to the
                // current stack-pointer value (SP).
                let mut nsaa: usize = 0; // sp

                // Pre-padding and extension of arguments
                for _arg in arguments.iter() {
                    // B.1: If the argument type is a Pure Scalable Type, no
                    // change is made at this stage
                    // NOTE: All of the types are U64, so nothing is done in
                    // this stage

                    // B.2: NA

                    // B.3: If the argument type is an HFA or an HVA, then the
                    // argument is used unmodified.

                    // B.4: If the argument type is a Composite Type that is
                    // larger than 16 bytes, then the argument is copied to memory
                    // allocated by the caller and the argument is replaced by a
                    // pointer to the copy.

                    // B.5: If the argument type is a Composite Type then the
                    // size of the argument is rounded up to the nearest multiple
                    // of 8 bytes.

                    // B.6: NA
                }

                // Assignment of arguments to registers and stack
                for (i, (arg, layout)) in arguments.iter().enumerate() {
                    // variadics spill onto the stack?
                    if i >= *named {
                        ngrn = 8;
                    }

                    // C.1-C.8 deal with vectors

                    // C.9: If the argument is an Integral or Pointer Type, the
                    // size of the argument is less than or equal to 8 bytes and
                    // the NGRN is less than 8, the argument is copied to the
                    // least significant bits in x[NGRN]. The NGRN is incremented
                    // by one. The argument has now been allocated.
                    if !layout.composite && layout.size <= 8 && ngrn < 8 {
                        load(&mut asm, &mut data, stack, ngrn as u8, *arg);
                        ngrn += 1;
                        continue;
                    }

                    // C.10: If the argument has an alignment of 16 then the
                    // NGRN is rounded up to the next even number.
                    if layout.align == 16 {
                        ngrn = ngrn.next_multiple_of(2);
                    }

                    // C.11 describes large integral types

                    // C.12: If the argument is a Composite Type and the size in
                    // double-words of the argument is not more than 8 minus NGRN,
                    // then the argument is copied into consecutive general-purpose
                    // registers, starting at x[NGRN]. The argument is passed as
                    // though it had been loaded into the registers from a double
                    // -word-aligned address with an appropriate sequence of LDR
                    // instructions loading consecutive registers from memory
                    // (the contents of any unused parts of the registers are
                    // unspecified by this standard). The NGRN is incremented by
                    // the number of registers used. The argument has now been
                    // allocated
                    if layout.composite && layout.size.div_ceil(8) <= 8 - ngrn {
                        assert!(!layout.composite, "composites not implemented");
                        continue;
                    }

                    // C.13: The NGRN is set to 8.
                    ngrn = 8;

                    // C.14: The NSAA is rounded up to the larger of 8 or the
                    // Natural Alignment of the argument’s type.
                    nsaa = nsaa.next_multiple_of(8.max(layout.align));

                    // C.15: If the argument is a composite type then the argument
                    // is copied to memory at the adjusted NSAA. The NSAA is
                    // incremented by the size of the argument. The argument has
                    // now been allocated.
                    if layout.composite {
                        assert!(!layout.composite, "composites not implemented");
                        assert_eq!(layout.align, 8);
                        assert_eq!(layout.size, 8);
                        load(&mut asm, &mut data, stack, sa, *arg);
                        asm.push_str(&format!("\tstr x{sa}, [sp, {nsaa}]\n"));
                        nsaa += layout.size;
                        continue;
                    }

                    // C.16: If the size of the argument is less than 8 bytes then
                    // the size of the argument is set to 8 bytes. The effect is
                    // as if the argument was copied to the least significant bits
                    // of a 64-bit register and the remaining bits filled with
                    // unspecified values.
                    let size = if layout.size < 8 { 8 } else { layout.size };

                    // C.17: The argument is copied to memory at the adjusted NSAA.
                    // The NSAA is incremented by the size of the argument. The
                    // argument has now been allocated.
                    assert_eq!(size, 8);
                    load(&mut asm, &mut data, stack, sa, *arg);
                    asm.push_str(&format!("\tstr x{sa}, [sp, {nsaa}]\n"));
                    nsaa += size;
                }

                let stack_size = nsaa.next_multiple_of(16);
                if stack_size > 0 {
                    asm.insert_str(allocate_stack, &format!("\tsub sp, sp, {stack_size}\n"));
                }
                asm.push_str(&format!("\tbl {symbol}\n"));
                if stack_size > 0 {
                    asm.push_str(&format!("\tadd sp, sp, {stack_size}\n"));
                }

                assert!(results.len() <= 1);
                for (arg, layout) in results.iter() {
                    // If the parameter passing algorithm above would have packed
                    // the return value type into registers then it will use the
                    // same algorithm. Otherwise, a pointer to the value is passed
                    // in x8.
                    //
                    // Therefore, to be compliant with the spec, only the first
                    // value will be packed, and the remaining values will be stored
                    // in the address pointed to by x8.
                    assert!(!layout.composite);
                    assert_eq!(layout.size, 8);
                    assert_eq!(layout.align, 8);
                    store_reg(&mut asm, stack, *arg, 0);
                }
            }
            Ir::LoadArguments { arguments } => {
                let stack = stack.unwrap();
                // Performs the reverse of the algorithm above.
                let mut ngrn: usize = 0;
                let mut nsaa: usize = 0;
                let nsaa_offset = callee_stack_offset;
                for (arg, layout) in arguments.iter() {
                    // C.9
                    if !layout.composite && layout.size <= 8 && ngrn < 8 {
                        store_reg(&mut asm, stack, *arg, ngrn as u8);
                        ngrn += 1;
                        continue;
                    }
                    // // C.10
                    // if layout.align == 16 {
                    //     ngrn = ngrn.next_multiple_of(2);
                    // }
                    // C.13
                    ngrn = 8;
                    // C.14
                    nsaa = nsaa.next_multiple_of(8.max(layout.align));
                    // C.16
                    let size = if layout.size < 8 { 8 } else { layout.size };
                    // C.17
                    assert_eq!(size, 8);
                    asm.push_str(&format!("\tldr x{sa}, [sp, {}]\n", nsaa + nsaa_offset));
                    store_reg(&mut asm, stack, *arg, sa);
                    nsaa += size;
                }
            }
            Ir::Return {
                results,
                deallocate,
            } => {
                assert!(results.len() <= 1);
                for (result, layout) in results.iter() {
                    assert!(!layout.composite);
                    assert_eq!(layout.size, 8);
                    assert_eq!(layout.align, 8);
                    load(&mut asm, &mut data, stack.unwrap(), 0, *result);
                }
                // TODO: This makes the codegen work, but I don't like how implicit
                // this structure is.
                stack = None;
                let stack_size = deallocate.next_multiple_of(16);
                asm.push_str(&format!("\tldp x29, x30, [sp, {stack_size}]\n"));
                asm.push_str(&format!("\tadd sp, sp, {}\n", stack_size + 16));
                asm.push_str("\tret\n");
            }
        }
    }
    if !data.map.is_empty() {
        asm.push_str(".data\n");
        for (data, label) in data.map.iter() {
            asm.push_str(&format!(
                "\t{label}: .asciz \"{}\"\n",
                match data {
                    Data::Str(str) => str,
                }
            ));
        }
    }
    asm
}
