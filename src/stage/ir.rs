use crate::error::{ErrorKind, Report};
use crate::stage::Stage;
use crate::stage::tokenize::Span;
use crate::tree::{If, *};
use bevy_app::prelude::*;
use bevy_ecs::error::Result;
use bevy_ecs::prelude::*;
use bevy_state::state::OnEnter;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

pub fn plugin(app: &mut App) {
    app.init_resource::<Literals>().add_systems(
        OnEnter(Stage::Ir),
        (
            (layout_types, allocate_literals, insert_proc_epilogue),
            allocate_declarations,
            allocate_variables,
            check_not_args,
            (
                if_ir,
                while_ir,
                return_ir,
                block_ir,
                declaration_ir,
                call_ir,
                bin_op_ir,
                unary_op_ir,
            ),
            proc_ir,
            super::next_stage,
        )
            .chain()
            .in_set(IrSystems),
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct IrSystems;

#[derive(Debug, Clone, Component)]
pub enum Ir {
    Store {
        dst: Entity,
        src: Entity,
    },
    Unary {
        dst: Entity,
        src: Entity,
        una: UnaryOp,
    },
    Bin {
        dst: Entity,
        lhs: Entity,
        rhs: Entity,
        bin: IrBinOp,
    },
    Label {
        label: Entity,
    },
    Jump {
        label: Entity,
    },
    JumpZero {
        label: Entity,
        arg: Entity,
    },
    JumpNotZero {
        label: Entity,
        arg: Entity,
    },
    Allocate {
        args: Vec<Entity>,
    },
    LoadArguments {
        arguments: Vec<Entity>,
    },
    Call {
        proc: Entity,
        arguments: Vec<Entity>,
        returns: Vec<Entity>,
    },
    ReturnAndDeallocate {
        returns: Vec<Entity>,
    },
}

#[derive(Default, Resource)]
pub struct Literals {
    pub map: HashMap<Literal, usize>,
    pub storage: Vec<Literal>,
}

impl Literals {
    pub fn allocate(&mut self, literal: Literal) -> usize {
        *self.map.entry(literal).or_insert_with(|| {
            let index = self.storage.len();
            self.storage.push(literal);
            index
        })
    }
}

#[derive(Debug, Clone, Copy, Component)]
pub enum Arg {
    Var(usize),
    Lit(usize),
    Const(usize),
}

impl Arg {
    pub fn allocate() -> Self {
        static UNIQUE: AtomicUsize = AtomicUsize::new(0);
        Self::Var(UNIQUE.fetch_add(1, Ordering::Relaxed))
    }

    pub fn allocate_const() -> Self {
        static UNIQUE: AtomicUsize = AtomicUsize::new(0);
        Self::Const(UNIQUE.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Copy, Component)]
pub struct Label(pub &'static str);

#[derive(Component)]
pub struct Unique;

#[derive(Component)]
pub struct Prologue;

#[derive(Component)]
pub struct Body;

#[derive(Component)]
pub struct Epilogue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrBinOp {
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

impl IrBinOp {
    pub fn expect_from_ast_bin_op_kind(kind: BinOp) -> Self {
        match kind {
            BinOp::Add => Self::Add,
            BinOp::Sub => Self::Sub,
            BinOp::Mul => Self::Mul,
            BinOp::Div => Self::Div,
            BinOp::Mod => Self::Mod,
            BinOp::Eq => Self::Eq,
            BinOp::Ne => Self::Ne,
            BinOp::Gt => Self::Gt,
            BinOp::Ge => Self::Ge,
            BinOp::Lt => Self::Lt,
            BinOp::Le => Self::Le,
            BinOp::BitAnd => Self::BitAnd,
            BinOp::BitOr => Self::BitOr,
            BinOp::Xor => Self::Xor,
            BinOp::Shr => Self::Shr,
            BinOp::Shl => Self::Shl,
            _ => unreachable!(),
        }
    }
}

#[derive(Component)]
#[relationship_target(relationship = IrOf)]
pub struct Irs(Vec<Entity>);

#[derive(Component)]
#[relationship(relationship_target = Irs)]
pub struct IrOf(pub Entity);

#[derive(Component)]
pub struct IrSub(pub Entity);

#[macro_export]
macro_rules! ir {
    [$($ir:expr),*$(,)?] => {
        bevy_ecs::related!($crate::stage::ir::Irs [$($ir),*])
    };
}

fn layout_types(mut commands: Commands, types: Query<(Entity, &Type), Without<Layout>>) {
    for (entity, ty) in types.iter() {
        let layout = match ty {
            Type::U64 => Layout {
                size: 8,
                align: 8,
                composite: false,
            },
            Type::Str => Layout {
                size: 8,
                align: 8,
                composite: false,
            },
            // TODO: The codegen should never use a `Not`, but this will also
            // cause it to just fail when it fetches the Layout of an arg...
            Type::Not => continue,
        };
        commands.entity(entity).insert(layout);
    }
}

fn allocate_literals(
    mut commands: Commands,
    literal_query: Query<(Entity, &Literal), With<Expr>>,
    mut literals: ResMut<Literals>,
) {
    for (entity, lit) in literal_query.iter() {
        let id = literals.allocate(*lit);
        commands.entity(entity).insert(Arg::Lit(id));
    }
}

fn check_not_args(
    mut commands: Commands,
    args: Query<(Entity, &Type, Has<Call>), With<Arg>>,
    spans: Query<&Span>,
) -> Result {
    for (entity, ty, is_call) in args.iter() {
        if *ty == Type::Not {
            if is_call {
                commands.entity(entity).remove::<Arg>();
            } else {
                let span = spans.get(entity)?;
                return Err(span.msg(format!("Expected value, got `{ty}`")).into());
            }
        }
    }
    Ok(())
}

fn allocate_declarations(
    mut commands: Commands,
    decls: Query<Entity, (With<Declaration>, Without<Const>)>,
    const_decls: Query<Entity, (With<Declaration>, With<Const>)>,
) {
    for entity in decls.iter() {
        commands.entity(entity).insert(Arg::allocate());
    }
    for entity in const_decls.iter() {
        commands.entity(entity).insert(Arg::allocate_const());
    }
}

fn allocate_variables(
    mut commands: Commands,
    decls: Query<&Arg>,
    variables: Query<(Entity, &VariableOf)>,
) -> Result {
    for (entity, var_of) in variables.iter() {
        commands.entity(entity).insert(*decls.get(var_of.0)?);
    }
    Ok(())
}

fn insert_proc_epilogue(mut commands: Commands, procs: Query<(Entity, &Ident), With<Proc>>) {
    for (entity, ident) in procs.iter() {
        // Insert the Epilogue into the proc such that subnodes can iterate
        // ancestors and find the correct epilogue.
        commands.entity(entity).insert((Label(ident.0), Epilogue));
    }
}

fn proc_ir(
    mut commands: Commands,
    mut proc: Query<(Entity, &Proc, &Ident, Option<&Args>, Option<&Returns>)>,
    children: Query<&Children>,
    arg_entity: Query<&Arg>,
) -> Result {
    for (entity, proc, ident, args, returns) in proc.iter_mut() {
        let label = Label(ident.0);
        let prologue = commands.spawn((ChildOf(entity), label, Prologue)).id();
        if let Some(proc_body) = proc.body {
            let body = commands.spawn((ChildOf(entity), label, Body)).id();
            // see `insert_proc_epilogue`
            let epilogue = entity;

            // Determines all the stack allocations necessary for this procedure.
            // TODO: This will also read any locally scoped functions, constants,
            // etc...
            let allocations = children
                .iter_descendants_depth_first(entity)
                .filter(|e| arg_entity.get(*e).is_ok_and(|a| matches!(a, Arg::Var(_))))
                .collect();

            commands.spawn((Ir::Label { label: prologue }, IrOf(entity)));
            commands.spawn((Ir::Allocate { args: allocations }, IrOf(entity)));
            if let Some(args) = args {
                commands.spawn((
                    Ir::LoadArguments {
                        arguments: args.to_vec(),
                    },
                    IrOf(entity),
                ));
            }
            commands.spawn((Ir::Label { label: body }, IrOf(entity)));
            commands.spawn((IrSub(proc_body), IrOf(entity)));
            commands.spawn((Ir::Label { label: epilogue }, IrOf(entity)));
            commands.spawn((
                Ir::ReturnAndDeallocate {
                    returns: returns.map(|r| r.to_vec()).unwrap_or_default(),
                },
                IrOf(entity),
            ));
        }
    }
    Ok(())
}

fn if_ir(mut commands: Commands, ifs: Query<(Entity, &If)>) {
    for (entity, iff) in ifs.iter() {
        let condition = commands.spawn((Unique, Label("if_condition"))).id();
        let body = commands.spawn((Unique, Label("if_body"))).id();
        let skip = commands.spawn((Unique, Label("if_skip"))).id();

        commands.entity(entity).insert(ir![
            Ir::Label { label: condition },
            IrSub(iff.condition),
            Ir::JumpZero {
                label: skip,
                arg: iff.condition,
            },
            Ir::Label { label: body },
            IrSub(iff.body),
            Ir::Label { label: skip },
        ]);
    }
}

fn while_ir(mut commands: Commands, whiles: Query<(Entity, &While)>) {
    for (entity, whilee) in whiles.iter() {
        let condition = commands.spawn((Unique, Label("while_condition"))).id();
        let body = commands.spawn((Unique, Label("while_body"))).id();
        let exit = commands.spawn((Unique, Label("while_exit"))).id();

        commands.entity(entity).insert(ir![
            Ir::Label { label: condition },
            IrSub(whilee.condition),
            Ir::JumpZero {
                label: exit,
                arg: whilee.condition,
            },
            Ir::Label { label: body },
            IrSub(whilee.body),
            Ir::Jump { label: condition },
            Ir::Label { label: exit },
        ]);
    }
}

fn return_ir(
    mut commands: Commands,
    returns: Query<(Entity, &Return, &Span)>,
    return_args: TreeQuery<&Returns>,
    epilogues: TreeQuery<Entity, With<Epilogue>>,
) -> Result {
    'outer: for (entity, ret, span) in returns.iter() {
        let epilogue = epilogues
            .first_ancestor(entity)
            .map_err(|_| span.custom("Nothing to return from"))?;
        let returns = return_args
            .first_ancestor(entity)
            .map_err(|_| span.kind(ErrorKind::NoReturns))?;

        assert_eq!(returns.len(), ret.expr.is_some() as usize);
        if let Some(expr) = ret.expr {
            commands.entity(entity).insert(ir![
                IrSub(expr),
                Ir::Store {
                    dst: returns[0],
                    src: expr,
                },
                Ir::Jump { label: epilogue },
            ]);
        } else {
            commands
                .entity(entity)
                .insert(ir![Ir::Jump { label: epilogue }]);
        }
        continue 'outer;
    }
    Ok(())
}

fn block_ir(mut commands: Commands, blocks: Query<(Entity, &Children), With<Block>>) {
    for (entity, children) in blocks.iter() {
        for child in children.iter() {
            commands.spawn((IrOf(entity), IrSub(child)));
        }
    }
}

fn declaration_ir(mut commands: Commands, decls: Query<(Entity, &Declaration), Without<Const>>) {
    for (entity, decl) in decls.iter() {
        if let Some(expr) = decl.expr {
            commands.entity(entity).insert(ir![
                IrSub(expr),
                Ir::Store {
                    dst: entity,
                    src: expr
                }
            ]);
        }
    }
}

fn call_ir(
    mut commands: Commands,
    calls: Query<(Entity, &Ident, Option<&Children>, &Span, &Type), With<Call>>,
    procs: Query<(Entity, &Ident), With<Proc>>,
) -> Result {
    for (entity, ident, args, span, ty) in calls.iter() {
        let proc = procs
            .iter()
            .find_map(|(e, i)| (i.0 == ident.0).then_some(e))
            .ok_or(span.custom("Undefined procedure"))?;
        let returns = if *ty != Type::Not {
            commands.entity(entity).insert(Arg::allocate());
            vec![entity]
        } else {
            Vec::new()
        };
        if let Some(args) = args {
            for arg in args.iter() {
                commands.spawn((IrOf(entity), IrSub(arg)));
            }
            commands.spawn((
                IrOf(entity),
                Ir::Call {
                    proc,
                    arguments: args.to_vec(),
                    returns,
                },
            ));
        } else {
            commands.entity(entity).insert(ir![Ir::Call {
                proc,
                arguments: Vec::new(),
                returns,
            }]);
        }
    }
    Ok(())
}

fn bin_op_ir(mut commands: Commands, bin_ops: Query<(Entity, &Children, &BinOp), Without<Const>>) {
    for (entity, children, bin) in bin_ops.iter() {
        let lhs = children[0];
        let rhs = children[1];
        match bin {
            BinOp::And => {
                let skip = commands.spawn((Unique, Label("and_skip"))).id();
                commands.entity(entity).insert((
                    Arg::allocate(),
                    ir![
                        IrSub(lhs),
                        Ir::Store {
                            dst: entity,
                            src: lhs
                        },
                        Ir::JumpZero {
                            label: skip,
                            arg: lhs
                        },
                        IrSub(rhs),
                        Ir::Store {
                            dst: entity,
                            src: rhs
                        },
                        Ir::Label { label: skip }
                    ],
                ));
            }
            BinOp::Or => {
                let skip = commands.spawn((Unique, Label("or_skip"))).id();
                commands.entity(entity).insert((
                    Arg::allocate(),
                    ir![
                        IrSub(lhs),
                        Ir::Store {
                            dst: entity,
                            src: lhs
                        },
                        Ir::JumpNotZero {
                            label: skip,
                            arg: lhs
                        },
                        IrSub(rhs),
                        Ir::Store {
                            dst: entity,
                            src: rhs
                        },
                        Ir::Label { label: skip }
                    ],
                ));
            }
            BinOp::Assign
            | BinOp::AddAssign
            | BinOp::SubAssign
            | BinOp::MulAssign
            | BinOp::DivAssign
            | BinOp::ModAssign
            | BinOp::BitAndAssign
            | BinOp::BitOrAssign
            | BinOp::XorAssign
            | BinOp::ShlAssign
            | BinOp::ShrAssign => {
                if *bin == BinOp::Assign {
                    commands.entity(entity).insert((
                        Arg::allocate(),
                        ir![IrSub(lhs), IrSub(rhs), Ir::Store { dst: lhs, src: rhs },],
                    ));
                } else {
                    let bin = match bin {
                        BinOp::AddAssign => IrBinOp::Add,
                        BinOp::SubAssign => IrBinOp::Sub,
                        BinOp::MulAssign => IrBinOp::Mul,
                        BinOp::DivAssign => IrBinOp::Div,
                        BinOp::ModAssign => IrBinOp::Mod,
                        BinOp::BitAndAssign => IrBinOp::BitAnd,
                        BinOp::BitOrAssign => IrBinOp::BitOr,
                        BinOp::XorAssign => IrBinOp::Xor,
                        BinOp::ShlAssign => IrBinOp::Shl,
                        BinOp::ShrAssign => IrBinOp::Shr,
                        _ => unreachable!(),
                    };
                    commands.entity(entity).insert((
                        Arg::allocate(),
                        ir![
                            IrSub(lhs),
                            IrSub(rhs),
                            Ir::Bin {
                                dst: lhs,
                                lhs,
                                rhs,
                                bin,
                            },
                        ],
                    ));
                }
            }
            _ => {
                commands.entity(entity).insert((
                    Arg::allocate(),
                    ir![
                        IrSub(lhs),
                        IrSub(rhs),
                        Ir::Bin {
                            dst: entity,
                            lhs,
                            rhs,
                            bin: IrBinOp::expect_from_ast_bin_op_kind(*bin)
                        }
                    ],
                ));
            }
        }
    }
}

fn unary_op_ir(mut commands: Commands, unary_ops: Query<(Entity, &Children, &UnaryOp)>) {
    for (entity, children, unary) in unary_ops.iter() {
        let expr = children[0];
        match unary {
            UnaryOp::Not => {
                commands.entity(entity).insert((
                    Arg::allocate(),
                    ir![
                        IrSub(expr),
                        Ir::Unary {
                            dst: entity,
                            src: expr,
                            una: *unary,
                        }
                    ],
                ));
            }
        }
    }
}
