use crate::stage::{Stage, codegen::Prog, ir::*};
use crate::{Flags, tree::*};
use bevy_app::prelude::*;
use bevy_ecs::{prelude::*, system::SystemParam};
use bevy_state::state::OnEnter;
use std::{collections::HashMap, io::Write};

pub fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(Stage::Codegen),
        (
            super::collect_ir_into_prog,
            (data_section, resolve_labels),
            asm,
            output,
            crate::stage::next_stage,
        )
            .chain(),
    );
}

#[derive(Component)]
struct AsmLabel(String);

impl std::fmt::Display for AsmLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Component)]
struct DataSection(String);

fn data_section(
    mut commands: Commands,
    prog: Single<Entity, With<Prog>>,
    literals: Res<Literals>,
    literal_query: Query<(Entity, &Arg), With<Literal>>,
) -> Result {
    let mut asm = String::from(".data\n");
    for (i, literal) in literals.storage.iter().enumerate() {
        match literal {
            Literal::Str(str) => {
                asm.push_str(&format!("\td{i}: .asciz \"{str}\"\n"));
            }
            Literal::Integer(_) => {}
        }
    }
    commands.entity(*prog).insert(DataSection(asm));
    for (entity, arg) in literal_query.iter() {
        let Arg::Lit(literal) = arg else {
            return Err("`Literal` `Arg` is not literal".into());
        };
        commands
            .entity(entity)
            .insert(AsmLabel(format!("d{literal}")));
    }
    Ok(())
}

fn resolve_labels(
    mut commands: Commands,
    labels: Query<(Entity, &Label, Has<Body>, Has<Epilogue>, Has<Unique>)>,
) {
    for (entity, label, body, epilogue, unique) in labels.iter() {
        let mut postfix = String::with_capacity(32);
        if body {
            postfix.push_str("_body");
        }
        if epilogue {
            postfix.push_str("_epilogue");
        }
        if unique {
            postfix.push_str(&format!("_{entity}"));
        }
        commands
            .entity(entity)
            .insert(AsmLabel(format!("_{}{postfix}", label.0)));
    }
}

#[derive(SystemParam)]
pub struct AsmQuery<'w, 's> {
    args: Query<'w, 's, &'static Arg>,
    literal_query: Query<'w, 's, &'static AsmLabel>,
    literals: Res<'w, Literals>,
}

fn load(
    asm: &mut String,
    query: &AsmQuery,
    stack: &HashMap<usize, usize>,
    dst: u8,
    src: Entity,
) -> Result {
    match query.args.get(src).map_err(|_| "no load src arg")? {
        Arg::Var(id) => {
            let offset = stack.get(id).ok_or("var unallocated")?;
            asm.push_str(&format!("\tldr x{dst}, [x29, -{offset}]\n"));
        }
        Arg::Lit(id) => match query.literals.storage[*id] {
            Literal::Str(_) => {
                let label = query
                    .literal_query
                    .get(src)
                    .map_err(|_| "no str asm label")?;
                asm.push_str(&format!("\tadrp x{dst}, {label}@PAGE\n"));
                asm.push_str(&format!("\tadd x{dst}, x{dst}, {label}@PAGEOFF\n"));
            }
            Literal::Integer(integer) => {
                if integer.bit_width() <= 12 {
                    asm.push_str(&format!("\tmov x{dst}, {integer}\n"));
                } else {
                    asm.push_str(&format!("\tldr x{dst}, ={integer}\n"));
                }
            }
        },
        Arg::Const(_) => {
            todo!("arg const");
        }
    }
    Ok(())
}

fn store_reg(
    asm: &mut String,
    query: &AsmQuery,
    stack: &HashMap<usize, usize>,
    dst: Entity,
    src: u8,
) -> Result {
    match query
        .args
        .get(dst)
        .map_err(|_| format!("no store dst arg {dst}"))?
    {
        Arg::Var(id) => {
            let offset = stack.get(id).ok_or("var unallocated")?;
            asm.push_str(&format!("\tstr x{src}, [x29, -{offset}]\n"));
            Ok(())
        }
        _ => Err("can only store into var".into()),
    }
}

fn store(
    asm: &mut String,
    query: &AsmQuery,
    stack: &HashMap<usize, usize>,
    dst: Entity,
    src: Entity,
) -> Result {
    let reg = 9;
    load(asm, query, stack, reg, src)?;
    store_reg(asm, query, stack, dst, reg)?;
    Ok(())
}

#[derive(Component)]
struct Asm(String);

fn asm(
    mut commands: Commands,
    prog: Single<(Entity, &Prog, &DataSection)>,
    query: AsmQuery,
    layouts: Query<&Layout>,
    procs: Query<(&Children, Option<&Args>), With<Proc>>,
    args: Query<&Arg>,
    labels: Query<&AsmLabel>,
    prologues: Query<&AsmLabel, With<Prologue>>,
) -> Result {
    let (entity, prog, data_section) = prog.into_inner();
    // println!("{:#?}", prog.0);
    let mut asm = String::new();
    asm.push_str(".global _start\n");
    asm.push_str("_start:\n");
    asm.push_str("\tbl _main\n");
    asm.push_str("\tb _exit\n");
    let mut stack = None;
    let mut stack_size = 0;
    let mut callee_stack_offset = 0;
    // temp r9..r15
    let sa = 9;
    let sb = 10;
    let sc = 11;
    for op in prog.0.iter() {
        match op {
            Ir::Store { dst, src } => {
                store(&mut asm, &query, stack.as_ref().unwrap(), *dst, *src)?;
            }
            Ir::Unary { dst, src, una } => match una {
                UnaryOp::Not => {
                    load(&mut asm, &query, stack.as_ref().unwrap(), sa, *src)?;
                    asm.push_str(&format!("\tmvn x{sa}, x{sa}\n"));
                    store_reg(&mut asm, &query, stack.as_ref().unwrap(), *dst, sa)?;
                }
            },
            Ir::Bin { dst, lhs, rhs, bin } => {
                load(&mut asm, &query, stack.as_ref().unwrap(), sb, *lhs)?;
                load(&mut asm, &query, stack.as_ref().unwrap(), sc, *rhs)?;
                match bin {
                    IrBinOp::Add
                    | IrBinOp::Sub
                    | IrBinOp::Mul
                    | IrBinOp::Div
                    | IrBinOp::BitAnd
                    | IrBinOp::BitOr
                    | IrBinOp::Shr
                    | IrBinOp::Shl
                    | IrBinOp::Xor => {
                        let op = match bin {
                            IrBinOp::Add => "add",
                            IrBinOp::Sub => "sub",
                            IrBinOp::Mul => "mul",
                            IrBinOp::Div => "udiv",
                            IrBinOp::BitAnd => "and",
                            IrBinOp::BitOr => "orr",
                            IrBinOp::Xor => "eor",
                            IrBinOp::Shr => "lsr",
                            IrBinOp::Shl => "lsl",
                            _ => unreachable!(),
                        };
                        asm.push_str(&format!("\t{op} x{sa}, x{sb}, x{sc}\n"));
                        store_reg(&mut asm, &query, stack.as_ref().unwrap(), *dst, sa)?;
                    }
                    IrBinOp::Eq
                    | IrBinOp::Ne
                    | IrBinOp::Gt
                    | IrBinOp::Lt
                    | IrBinOp::Ge
                    | IrBinOp::Le => {
                        let cond = match bin {
                            IrBinOp::Eq => "eq",
                            IrBinOp::Ne => "ne",
                            IrBinOp::Gt => "gt",
                            IrBinOp::Ge => "ge",
                            IrBinOp::Lt => "lt",
                            IrBinOp::Le => "le",
                            _ => unreachable!(),
                        };
                        asm.push_str(&format!("\tcmp x{sb}, x{sc}\n"));
                        asm.push_str(&format!("\tcset x{sa}, {cond}\n"));
                        store_reg(&mut asm, &query, stack.as_ref().unwrap(), *dst, sa)?;
                    }
                    IrBinOp::Mod => {
                        asm.push_str(&format!("\tudiv x{sa}, x{sb}, x{sc}\n"));
                        asm.push_str(&format!("\tmsub x{sa}, x{sa}, x{sc}, x{sb}\n"));
                        store_reg(&mut asm, &query, stack.as_ref().unwrap(), *dst, sa)?;
                    }
                }
            }
            Ir::Label { label } => {
                let label = labels.get(*label).map_err(|_| "no asm label")?;
                asm.push_str(&format!("{label}:\n"));
            }
            Ir::Jump { label } => {
                let label = labels.get(*label).map_err(|_| "no asm jump label")?;
                asm.push_str(&format!("\tb {label}\n"));
            }
            Ir::JumpZero { label, arg } => {
                let label = labels.get(*label).map_err(|_| "no asm jump zero label")?;
                load(&mut asm, &query, stack.as_ref().unwrap(), sa, *arg)?;
                asm.push_str(&format!("\tcbz x{sa}, {label}\n"));
            }
            Ir::JumpNotZero { label, arg } => {
                let label = labels
                    .get(*label)
                    .map_err(|_| "no asm jump not zero label")?;
                load(&mut asm, &query, stack.as_ref().unwrap(), sa, *arg)?;
                asm.push_str(&format!("\tcbnz x{sa}, {label}\n"));
            }
            Ir::Allocate { args: arguments } => {
                let mut offset: usize = 8;
                let mut s = HashMap::new();
                for arg_entity in arguments.iter() {
                    let arg = args.get(*arg_entity).map_err(|_| "no allocate arg")?;
                    let Arg::Var(index) = arg else {
                        return Err("can only allocate vars".into());
                    };
                    if s.contains_key(index) {
                        continue;
                    }
                    let layout = layouts
                        .get(*arg_entity)
                        .map_err(|_| format!("no allocate arg layout {arg_entity}"))?;
                    offset = offset.next_multiple_of(layout.align);
                    assert!(s.insert(*index, offset).is_none());
                    offset += layout.size;
                }
                let size = offset.next_multiple_of(16);
                stack_size = size;
                callee_stack_offset = size + 16;
                stack = Some(s);
                asm.push_str(&format!("\tsub sp, sp, {}\n", callee_stack_offset));
                asm.push_str(&format!("\tstp x29, x30, [sp, {stack_size}]\n"));
                asm.push_str(&format!("\tadd x29, sp, {stack_size}\n"));
            }
            Ir::Call {
                proc,
                arguments,
                returns,
            } => {
                let (children, args) = procs.get(*proc).map_err(|_| "no call proc")?;
                let label = prologues
                    .iter_many(children)
                    .next()
                    .ok_or("no proc prologue label")?;
                let symbol = &label.0;
                let named = args.map(|a| a.len()).unwrap_or(0);

                let stack = stack.as_ref().unwrap();
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
                for (i, arg) in arguments.iter().enumerate() {
                    let layout = layouts.get(*arg).map_err(|_| "no call arg layout")?;

                    // variadics spill onto the stack?
                    if i >= named {
                        ngrn = 8;
                    }

                    // C.1-C.8 deal with vectors

                    // C.9: If the argument is an Integral or Pointer Type, the
                    // size of the argument is less than or equal to 8 bytes and
                    // the NGRN is less than 8, the argument is copied to the
                    // least significant bits in x[NGRN]. The NGRN is incremented
                    // by one. The argument has now been allocated.
                    if !layout.composite && layout.size <= 8 && ngrn < 8 {
                        load(&mut asm, &query, stack, ngrn as u8, *arg)?;
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
                        load(&mut asm, &query, stack, sa, *arg)?;
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
                    load(&mut asm, &query, stack, sa, *arg)?;
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

                assert!(returns.len() <= 1);
                for arg in returns.iter() {
                    let layout = layouts.get(*arg).map_err(|_| "no call return layout")?;
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
                    store_reg(&mut asm, &query, stack, *arg, 0)?;
                }
            }
            Ir::LoadArguments { arguments } => {
                let stack = stack.as_ref().unwrap();
                // Performs the reverse of the algorithm above.
                let mut ngrn: usize = 0;
                let mut nsaa: usize = 0;
                let nsaa_offset = callee_stack_offset;
                for arg in arguments.iter() {
                    let layout = layouts.get(*arg).map_err(|_| "no load args layout")?;
                    // C.9
                    if !layout.composite && layout.size <= 8 && ngrn < 8 {
                        store_reg(&mut asm, &query, stack, *arg, ngrn as u8)?;
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
                    store_reg(&mut asm, &query, stack, *arg, sa)?;
                    nsaa += size;
                }
            }
            Ir::ReturnAndDeallocate { returns } => {
                assert!(returns.len() <= 1);
                for entity in returns.iter() {
                    // assert!(!layout.composite);
                    // assert_eq!(layout.size, 8);
                    // assert_eq!(layout.align, 8);
                    load(&mut asm, &query, stack.as_ref().unwrap(), 0, *entity)?;
                }
                // TODO: This makes the codegen work, but I don't like how implicit
                // this structure is.
                stack = None;
                asm.push_str(&format!("\tldp x29, x30, [sp, {stack_size}]\n"));
                asm.push_str(&format!("\tadd sp, sp, {callee_stack_offset}\n"));
                asm.push_str("\tret\n");
            }
        }
    }
    asm.push_str(&data_section.0);
    commands.entity(entity).insert(Asm(asm));
    Ok(())
}

fn output(flags: Single<&Flags>, asm: Single<&Asm>) -> Result {
    if flags.codegen {
        println!("{}", asm.0);
    }
    assemble_and_link(&asm.0, &flags.output())?;
    Ok(())
}

fn assemble_and_link(asm: &str, output: &str) -> Result {
    let obj = format!("{}.o", output.replace('/', "_"));
    let mut ass = std::process::Command::new("as")
        .args(["-o", &obj, "-"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    ass.stdin.take().unwrap().write_all(asm.as_bytes()).unwrap();
    let ass_out = ass.wait_with_output().unwrap();
    if !ass_out.status.success() {
        return Err("Assembler failed".into());
    }

    let ld_out = std::process::Command::new("ld")
        .args([
            "-o",
            output,
            "-e",
            "_start",
            "-lSystem",
            "-syslibroot",
            "/Applications/Xcode.app/Contents/Developer/Platforms/\
            MacOSX.platform/Developer/SDKs/MacOSX15.5.sdk",
            &obj,
        ])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

    let _ = std::fs::remove_file(&obj);
    ld_out.success().ok_or("Linker failed".into())
}
