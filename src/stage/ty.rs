use crate::stage::{Stage, ir::IrSystems, tokenize::Span};
use crate::{
    error::{ErrorKind, Report},
    tree::{If, *},
};
use bevy_app::prelude::*;
use bevy_ecs::{prelude::*, schedule::ScheduleLabel};
use bevy_state::state::OnEnter;

// NOTE:
//
// `Proc` arguments and returns are declarations that are assigned types in
// the parsing stage.
//
// Conditions for `If` and `While` blocks must be of _some_ kind of integer,
// not necessarily a word.
//
// Blocks are not expressions at the moment, so they remain untyped.
// TODO: Block should be expressions! Proc bodies could then use the
// same type checking!

pub fn plugin(app: &mut App) {
    app.init_resource::<Rerun>()
        .add_observer(assign_type)
        .add_systems(
            OnEnter(Stage::Type),
            (
                (
                    constrain_literal_types,
                    constrain_conditional_types,
                    constrain_bin_op_types,
                    constrain_unary_op_types,
                ),
                (assign_return_types, assign_decl_types, assign_call_types),
                crate::error::check_stage_error,
                propagate,
                super::next_stage,
            )
                .chain()
                .in_set(TypeSystems),
        )
        .configure_sets(Update, TypeSystems.before(IrSystems))
        .init_schedule(Typing)
        .add_systems(
            Typing,
            (
                propagate_unary_op_types,
                propagate_bin_op_types,
                resolve_integers,
                propagate_decls,
                constrain_ident_types,
            )
                .chain(),
        );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ScheduleLabel)]
pub struct Typing;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct TypeSystems;

#[derive(Default, Resource)]
struct Rerun(bool);

#[derive(EntityEvent)]
struct AssignType {
    entity: Entity,
    ty: Type,
    not_is_valid: bool,
}

fn assign_type(
    assign: On<AssignType>,
    mut commands: Commands,
    entities: Query<(Option<&Type>, Option<&VariableOf>, Has<IntegerLike>)>,
    decls: Query<&Type>,
    spans: Query<&Span>,
) -> Result {
    let (ty, var_of, integer_like) = entities.get(assign.entity)?;
    if let Some(ty) = ty
        && *ty != assign.ty
    {
        let span = spans.get(assign.entity)?;
        return Err(span
            .msg(format!("Expected `{}`, got `{ty}`", assign.ty))
            .into());
    }
    if !assign.ty.is_integer() && integer_like {
        let span = spans.get(assign.entity)?;
        return Err(span
            .msg(format!("Expected `{}`, got integer", assign.ty))
            .into());
    }
    if !assign.not_is_valid && assign.ty == Type::Not {
        let span = spans.get(assign.entity)?;
        return Err(span
            .msg(format!("Expected value, got `{}`", assign.ty))
            .into());
    }
    commands.entity(assign.entity).insert(assign.ty);
    if let Some(var_of) = var_of
        && let Ok(decl_ty) = decls.get(var_of.0)
        && *decl_ty != assign.ty
    {
        let span = spans.get(assign.entity)?;
        return Err(span
            .msg(format!("Expected `{decl_ty}`, got `{}`", assign.ty))
            .into());
    }
    Ok(())
}

#[derive(Component)]
struct IntegerLike;

fn constrain_conditional_types(mut commands: Commands, ifs: Query<&If>, whiles: Query<&While>) {
    for i in ifs.iter() {
        commands.entity(i.condition).insert(IntegerLike);
    }
    for w in whiles.iter() {
        commands.entity(w.condition).insert(IntegerLike);
    }
}

impl BinOp {
    fn boolean(&self) -> bool {
        matches!(self, Self::Eq | Self::Ne | Self::And | Self::Or)
    }
}

fn constrain_bin_op_types(mut commands: Commands, exprs: Query<(Entity, &BinOp, &Children)>) {
    for (entity, op, children) in exprs.iter() {
        if op.boolean() {
            commands.entity(entity).insert(IntegerLike);
            for child in children.iter() {
                commands.entity(child).insert(IntegerLike);
            }
        }
    }
}

fn constrain_unary_op_types(mut commands: Commands, exprs: Query<(Entity, &UnaryOp)>) {
    for (entity, op) in exprs.iter() {
        match op {
            UnaryOp::Not => {
                // TODO: `Not` should reflect the underlying type, but that would imply
                // relating types, which implies more complex resolution.
                commands.entity(entity).insert(IntegerLike);
            }
        }
    }
}

fn assign_return_types(
    mut commands: Commands,
    returns: Query<(Entity, &Return, &Span)>,
    procs: TreeQuery<&Returns, With<Proc>>,
    decls: Query<&Type>,
) -> Result {
    for (entity, r, span) in returns.iter() {
        let returns = procs
            .first_ancestor(entity)
            .map_err(|_| span.kind(ErrorKind::NoReturns))?;
        assert!(returns.len() <= 1);
        if returns.is_empty() {
            r.expr
                .is_none()
                .ok_or(span.custom("Expected 0 return values"))?;
        } else {
            let ty = decls.get(returns[0])?;
            let expr = r.expr.ok_or(span.custom("Expected 1 return value"))?;
            commands.entity(expr).trigger(|entity| AssignType {
                entity,
                ty: *ty,
                not_is_valid: false,
            });
        }
    }
    Ok(())
}

// NOTE: n^2 ident iteration!
fn constrain_ident_types(
    mut commands: Commands,
    decls: Query<(Entity, &Ident, Option<&Type>, &Declaration)>,
    idents: TreeQuery<(Entity, &Ident), With<Expr>>,
    integer_like: Query<&IntegerLike, Without<Type>>,
) -> Result {
    enum Bind {
        IntegerLike,
        Type(Type),
    }

    for (entity, ident, ty, decl) in decls.iter() {
        let bind = if let Some(ty) = ty {
            Bind::Type(*ty)
        } else if let Some(expr) = decl.expr
            && integer_like.contains(expr)
        {
            Bind::IntegerLike
        } else {
            continue;
        };

        // TODO: ordering is important here but it is not checked.
        for (usage, usage_ident) in idents.iter_descendants_of_parent(entity)? {
            if usage_ident.0 == ident.0 {
                match bind {
                    Bind::IntegerLike => {
                        commands.entity(usage).insert(IntegerLike);
                    }
                    Bind::Type(ty) => {
                        commands.entity(usage).trigger(|entity| AssignType {
                            entity,
                            ty,
                            not_is_valid: false,
                        });
                    }
                }
            }
        }
    }

    Ok(())
}

fn constrain_literal_types(mut commands: Commands, literals: Query<(Entity, &Literal)>) {
    for (entity, literal) in literals.iter() {
        match literal {
            Literal::Integer(_) => {
                commands.entity(entity).insert(IntegerLike);
            }
            Literal::Str(_) => {
                commands.entity(entity).trigger(|entity| AssignType {
                    entity,
                    ty: Type::Str,
                    not_is_valid: false,
                });
            }
        }
    }
}

fn assign_decl_types(mut commands: Commands, decls: Query<(&Type, &Declaration)>) {
    for (ty, decl) in decls.iter() {
        if let Some(expr) = decl.expr {
            commands.entity(expr).trigger(|entity| AssignType {
                entity,
                ty: *ty,
                not_is_valid: false,
            });
        }
    }
}

fn assign_call_types(
    mut commands: Commands,
    calls: Query<(Entity, &Ident, Option<&Children>, &Span), With<Call>>,
    procs: Query<(&Ident, Option<&Args>, Option<&Returns>, Has<Variadic>), With<Proc>>,
    decls: Query<&Type>,
    spans: Query<&Span>,
) -> Result {
    for (entity, ident, call_args, span) in calls.iter() {
        let (proc_args, proc_returns, variadic) = procs
            .iter()
            .find_map(|(i, a, r, v)| (i.0 == ident.0).then_some((a, r, v)))
            .ok_or(span.custom("Undefined procedure"))?;

        let num_call_args = call_args.map(|a| a.len()).unwrap_or(0);
        if let Some(proc_args) = proc_args {
            let num_proc_args = proc_args.len();
            if num_call_args < num_proc_args || (num_call_args > num_proc_args && !variadic) {
                let s = if num_proc_args == 1 { "" } else { "s" };
                return Err(span
                    .msg(format!(
                        "Expected {num_proc_args} argument{s}, got {num_call_args}"
                    ))
                    .into());
            }

            for (call, proc) in call_args.unwrap().iter().zip(proc_args.iter()) {
                let proc_span = spans.get(proc)?;
                let ty = decls
                    .get(proc)
                    .map_err(|_| proc_span.custom("Unknown type"))?;
                commands.entity(call).trigger(|entity| AssignType {
                    entity,
                    ty: *ty,
                    not_is_valid: false,
                });
            }
        } else {
            if num_call_args != 0 && !variadic {
                return Err(span
                    .msg(format!("Expected 0 arguments, got {num_call_args}"))
                    .into());
            }
        }

        if let Some(proc_returns) = proc_returns {
            assert!(proc_returns.len() <= 1);
            if let Some(proc_return) = proc_returns.first() {
                let proc_span = spans.get(*proc_return)?;
                let ty = decls
                    .get(*proc_return)
                    .map_err(|_| proc_span.custom("Unknown type"))?;
                commands.entity(entity).trigger(|entity| AssignType {
                    entity,
                    ty: *ty,
                    not_is_valid: false,
                });
            } else {
                commands.entity(entity).trigger(|entity| AssignType {
                    entity,
                    ty: Type::Not,
                    not_is_valid: true,
                });
            }
        } else {
            commands.entity(entity).trigger(|entity| AssignType {
                entity,
                ty: Type::Not,
                not_is_valid: true,
            });
        }
    }
    Ok(())
}

fn propagate(world: &mut World) -> Result {
    loop {
        world.resource_mut::<Rerun>().0 = false;
        world.run_schedule(Typing);
        world.run_system_cached(crate::error::check_stage_error)?;
        if !world.resource::<Rerun>().0 {
            break;
        }
    }
    Ok(())
}

fn propagate_bin_op_types(
    mut commands: Commands,
    expr: Query<(Entity, &Children, &BinOp, &Span, Has<Type>)>,
    terms: Query<Option<&Type>>,
    mut rerun: ResMut<Rerun>,
) -> Result {
    for (entity, children, op, span, typed) in expr.iter() {
        let lhs = terms.get(children[0])?;
        let rhs = terms.get(children[1])?;
        let apply_type = !typed && !op.boolean();
        match (lhs, rhs) {
            (Some(lhs), Some(rhs)) => {
                if lhs != rhs {
                    return Err(span
                        .msg(format!("Type mismatch: `{lhs}` and `{rhs}`"))
                        .into());
                }
                if apply_type {
                    commands.entity(entity).trigger(|entity| AssignType {
                        entity,
                        ty: *lhs,
                        not_is_valid: false,
                    });
                    rerun.0 = true;
                }
            }
            (Some(lhs), None) => {
                commands.entity(children[1]).trigger(|entity| AssignType {
                    entity,
                    ty: *lhs,
                    not_is_valid: false,
                });
                if apply_type {
                    commands.entity(entity).trigger(|entity| AssignType {
                        entity,
                        ty: *lhs,
                        not_is_valid: false,
                    });
                }
                rerun.0 = true;
            }
            (None, Some(rhs)) => {
                commands.entity(children[0]).trigger(|entity| AssignType {
                    entity,
                    ty: *rhs,
                    not_is_valid: false,
                });
                if apply_type {
                    commands.entity(entity).trigger(|entity| AssignType {
                        entity,
                        ty: *rhs,
                        not_is_valid: false,
                    });
                }
                rerun.0 = true;
            }
            (None, None) => {
                rerun.0 = true;
            }
        }
    }
    Ok(())
}

fn propagate_unary_op_types(
    mut commands: Commands,
    expr: Query<(Entity, &Children, Has<Type>), With<UnaryOp>>,
    terms: Query<Option<&Type>>,
    mut rerun: ResMut<Rerun>,
) -> Result {
    for (entity, children, typed) in expr.iter() {
        let expr = terms.get(children[0])?;
        let apply_type = !typed;
        match expr {
            Some(ty) => {
                if apply_type {
                    commands.entity(entity).trigger(|entity| AssignType {
                        entity,
                        ty: *ty,
                        not_is_valid: false,
                    });
                    rerun.0 = true;
                }
            }
            None => {
                rerun.0 = true;
            }
        }
    }
    Ok(())
}

fn propagate_decls(
    mut commands: Commands,
    decls: Query<(Entity, &Declaration, &Span), Without<Type>>,
    exprs: Query<&Type>,
    mut rerun: ResMut<Rerun>,
) -> Result {
    for (entity, decl, span) in decls.iter() {
        if let Some(expr) = decl.expr
            && let Ok(ty) = exprs.get(expr)
        {
            commands.entity(entity).trigger(|entity| AssignType {
                entity,
                ty: *ty,
                not_is_valid: false,
            });
        } else {
            if decl.expr.is_none() {
                return Err(span.custom("Expected assignment").into());
            }
            rerun.0 = true;
        }
    }
    Ok(())
}

fn resolve_integers(
    mut commands: Commands,
    entities: Query<Entity, (With<IntegerLike>, Without<Type>)>,
) {
    for entity in entities.iter() {
        commands.entity(entity).trigger(|entity| AssignType {
            entity,
            ty: Type::U64,
            not_is_valid: false,
        });
    }
}
