use crate::stage::{Stage, tokenize::Span};
use crate::{error::Report, tree::*};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_state::state::OnEnter;

// NOTE: This is probably incredibly inefficient, but it is a simple piece
// of code that mostly works.
pub fn plugin(app: &mut App) {
    app.add_observer(assign_type).add_systems(
        OnEnter(Stage::Type),
        // chained for consistent error reporting, also pretty strict
        // ordering required here...
        (
            verify_const_decls,
            assign_literal_types,
            assign_proc_types,
            assign_return_exprs,
            assign_variables,
            propagate_call_exprs,
            propagate_types_up,
            propagate_types_down,
            propagate_integer_defaults,
            (
                verify_typed,
                verify_condition,
                verify_literals,
                verify_bin_ops,
            ),
            super::next_stage,
        )
            .chain(),
    );
}

#[derive(EntityEvent)]
struct AssignType {
    entity: Entity,
    ty: Type,
}

fn assign_type(
    assign: On<AssignType>,
    mut commands: Commands,
    types_down: Query<&Children, With<TypePropagatesDown>>,
    types_up: Query<&ChildOf, With<TypePropagatesUp>>,
    typed: Query<Has<DontPropagateUpTo>, With<Typed>>,
    types: Query<&Type>,
    spans: Query<&Span>,
    decl: Query<(), With<VarDecl>>,
    var_ofs: Query<(Entity, &VariableOf)>,
) -> Result {
    if let Ok(ty) = types.get(assign.entity) {
        if *ty != assign.ty {
            let span = spans.get(assign.entity)?;
            return Err(span
                .msg(format!(
                    "Expression is `{}` but expected to be `{}`",
                    ty, assign.ty
                ))
                .into());
        }
        return Ok(());
    }
    commands.entity(assign.entity).insert(assign.ty);
    if let Ok((_, var_of)) = var_ofs.get(assign.entity) {
        commands.entity(var_of.0).trigger(|entity| AssignType {
            entity,
            ty: assign.ty,
        });
    }
    if decl.contains(assign.entity) {
        // TODO: VariableOf needs to be a relationship!
        for (entity, var_of) in var_ofs.iter() {
            if var_of.0 == assign.entity {
                commands.entity(entity).trigger(|entity| AssignType {
                    entity,
                    ty: assign.ty,
                });
            }
        }
    }
    if let Ok(children) = types_down.get(assign.entity) {
        for child in children.iter() {
            if typed.contains(child) {
                commands.entity(child).trigger(|entity| AssignType {
                    entity,
                    ty: assign.ty,
                });
            }
        }
    }
    if let Ok(parent) = types_up.get(assign.entity)
        && typed.get(parent.0).is_ok_and(|d| !d)
    {
        commands.entity(parent.0).trigger(|entity| AssignType {
            entity,
            ty: assign.ty,
        });
    }
    Ok(())
}

fn verify_const_decls(decls: Query<&Span, (With<Const>, Without<Type>, With<Decl>)>) -> Result {
    if let Some(span) = decls.iter().next() {
        return Err(span.custom("Constant requires explicit type").into());
    }
    Ok(())
}

fn assign_literal_types(mut commands: Commands, literals: Query<(Entity, &Literal)>) {
    for (entity, literal) in literals.iter() {
        match literal {
            Literal::Str(_) => {
                commands.entity(entity).insert(Type::Str);
            }
            Literal::Integer(_) => {}
        }
    }
}

fn assign_proc_types(
    mut commands: Commands,
    procs: Query<(Entity, &Returns), With<Proc>>,
    decls: Query<&Type>,
) -> Result {
    for (entity, returns) in procs.iter() {
        if let Some(returns) = returns.0 {
            let ty = decls.get(returns)?;
            commands.entity(entity).insert(*ty);
        } else {
            commands.entity(entity).insert(Type::Not);
        }
    }
    Ok(())
}

fn assign_return_exprs(
    mut commands: Commands,
    returns: Query<(Entity, &Span), With<Return>>,
    procs: TreeQuery<&Type, With<Proc>>,
) -> Result {
    for (entity, span) in returns.iter() {
        let ty = procs
            .first_ancestor(entity)
            .map_err(|_| span.custom("`return` is not in a procedure body"))?;
        commands.entity(entity).insert(*ty);
    }
    Ok(())
}

fn propagate_types_down(
    mut commands: Commands,
    types: Query<(&Type, &Children), With<TypePropagatesDown>>,
    typed: Query<(), With<Typed>>,
) {
    for (ty, children) in types.iter() {
        for child in children.iter() {
            if typed.contains(child) {
                commands
                    .entity(child)
                    .trigger(|entity| AssignType { entity, ty: *ty });
            }
        }
    }
}

fn propagate_types_up(
    mut commands: Commands,
    types: Query<(&Type, &ChildOf), With<TypePropagatesUp>>,
    typed: Query<Has<DontPropagateUpTo>, With<Typed>>,
) {
    for (ty, parent) in types.iter() {
        if typed.get(parent.0).is_ok_and(|d| !d) {
            commands
                .entity(parent.0)
                .trigger(|entity| AssignType { entity, ty: *ty });
        }
    }
}

fn assign_variables(
    mut commands: Commands,
    vars: Query<&Type, Or<(With<VarDecl>, With<ArgDecl>)>>,
    var_ofs: Query<(Entity, &VariableOf)>,
) {
    for (entity, var_of) in var_ofs.iter() {
        if let Ok(ty) = vars.get(var_of.0) {
            commands.entity(entity).insert(*ty);
        }
    }
}

fn propagate_call_exprs(
    mut commands: Commands,
    calls: Query<(Entity, &Ident, &CallArgs, &Span), With<Call>>,
    procs: Query<(&Ident, &Args, &Type, Has<Variadic>)>,
    types: Query<&Type>,
) -> Result {
    for (entity, call_ident, call_args, span) in calls.iter() {
        let (_, proc_args, proc_ty, variadic) = procs
            .iter()
            .find(|(i, _, _, _)| i.0 == call_ident.0)
            .ok_or(span.custom("Undefined procedure"))?;
        if call_args.len() < proc_args.len() || (call_args.len() > proc_args.len() && !variadic) {
            return Err(span
                .msg(format!(
                    "Expected {} arguments, got {}",
                    proc_args.len(),
                    call_args.len(),
                ))
                .into());
        }
        for (call_arg, proc_arg) in call_args.iter().zip(proc_args.iter()) {
            let ty = types.get(*proc_arg)?;
            commands
                .entity(*call_arg)
                .trigger(|entity| AssignType { entity, ty: *ty });
        }
        commands.entity(entity).trigger(|entity| AssignType {
            entity,
            ty: *proc_ty,
        });
    }
    Ok(())
}

fn propagate_integer_defaults(
    mut commands: Commands,
    literals: Query<(Entity, &Literal), Without<Type>>,
) {
    for (entity, literal) in literals.iter() {
        // TODO: check the literal for size and sign?
        if matches!(literal, Literal::Integer(_)) {
            commands.entity(entity).trigger(|entity| AssignType {
                entity,
                ty: Type::I32,
            });
        }
    }
}

fn verify_typed(
    not_typed: Query<Entity, (Without<Type>, With<Typed>)>,
    spans: Query<&Span>,
) -> Result {
    if let Some(entity) = not_typed.iter().next() {
        let span = spans.get(entity)?;
        return Err(span.custom("Failed to infer type").into());
    }
    Ok(())
}

fn verify_condition(
    conditions: Query<&Condition>,
    spans: Query<&Span>,
    types: Query<&Type>,
) -> Result {
    for condition in conditions.iter() {
        let span = spans.get(condition.0)?;
        let ty = types
            .get(condition.0)
            .map_err(|_| span.custom("Expected integer expression"))?;
        if !ty.is_integer() {
            return Err(span.msg(format!("Expected integer, got `{ty}`")).into());
        }
    }
    Ok(())
}

fn verify_literals(literals: Query<(&Literal, &Type, &Span)>) -> Result {
    for (literal, ty, span) in literals.iter() {
        match literal {
            Literal::Integer(_) => {
                if !ty.is_integer() {
                    return Err(span.msg(format!("Integer is not a valid `{ty}`")).into());
                }
            }
            Literal::Str(_) => {
                if !matches!(ty, Type::Str) {
                    return Err(span.msg(format!("String is not a valid `{ty}`")).into());
                }
            }
        }
    }
    Ok(())
}

fn verify_bin_ops(ops: Query<(&BinOp, &Type, &Span)>) -> Result {
    for (op, ty, span) in ops.iter() {
        if !matches!(op, BinOp::Assign) && !ty.is_integer() {
            return Err(span
                .msg(format!("`{op}` expects integer type, got `{ty}`"))
                .into());
        }
    }
    Ok(())
}
