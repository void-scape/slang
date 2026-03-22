use crate::{
    error::Report,
    stage::{
        Stage,
        tokenize::{Span, StaticSourceFile},
    },
    tree::*,
};
use bevy_app::prelude::*;
use bevy_ecs::{prelude::*, system::SystemParam};
use bevy_state::state::OnEnter;
use std::collections::HashMap;

pub fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(Stage::Variable),
        (
            walk_source_files,
            crate::error::check_stage_error,
            verify_all_variables_resolved,
            super::next_stage,
        )
            .chain(),
    );
}

#[derive(SystemParam)]
pub struct VarQuery<'w, 's> {
    children: Query<'w, 's, &'static Children>,
    variables: Query<
        'w,
        's,
        (
            Entity,
            &'static Ident,
            &'static Span,
            Has<Variable>,
            Has<Declaration>,
        ),
    >,
    new_scope: Query<'w, 's, (), With<Scope>>,
}

fn walk_source_files(
    mut commands: Commands,
    roots: Query<Entity, With<StaticSourceFile>>,
    query: VarQuery,
) -> Result {
    let mut scope = Vec::new();
    for root in roots.iter() {
        scope.push(HashMap::default());
        walk_source_file(&mut commands, &mut scope, &query, root)?;
        scope.clear();
    }
    Ok(())
}

fn walk_source_file(
    commands: &mut Commands,
    scope: &mut Vec<HashMap<&'static str, Entity>>,
    query: &VarQuery,
    root: Entity,
) -> Result {
    let new_scope = query.new_scope.contains(root);
    if new_scope {
        scope.push(HashMap::default());
    }
    let Ok(children) = query.children.get(root) else {
        return Ok(());
    };
    for child in children.iter() {
        if let Ok((entity, ident, span, is_var, is_decl)) = query.variables.get(child) {
            if is_var {
                if let Some(var) = scope.iter().flat_map(|m| m.get(ident.0)).next() {
                    commands.entity(entity).insert(VariableOf(*var));
                } else {
                    return Err(span.custom("Variable is not declared").into());
                }
            } else if is_decl {
                if scope.iter().any(|m| m.contains_key(ident.0)) {
                    return Err(span.custom("Variable redefinition").into());
                }
                scope.last_mut().unwrap().insert(ident.0, entity);
            }
        }
        walk_source_file(commands, scope, query, child)?;
    }
    if new_scope {
        scope.pop();
    }
    Ok(())
}

fn verify_all_variables_resolved(
    variables: Query<&Span, (With<Variable>, Without<VariableOf>)>,
) -> Result {
    if let Some(span) = variables.iter().next() {
        // TODO: NEEEED bulk reporting especially here
        return Err(span.custom("Variable is not declared").into());
    }
    Ok(())
}
