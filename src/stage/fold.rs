use crate::{
    error::Report,
    stage::{Stage, tokenize::Span},
    tree::*,
};
use bevy_app::prelude::*;
use bevy_ecs::{prelude::*, schedule::ScheduleLabel};
use bevy_state::state::OnEnter;
use std::ops::{BitAnd, BitOr, BitXor};

pub fn plugin(app: &mut App) {
    app.init_resource::<Rerun>()
        .add_message::<Folded>()
        .add_systems(
            OnEnter(Stage::Fold),
            (
                (mark_const_expr, mark_variables_const),
                verify_variable_const_assign,
                fold,
                verify_folded,
                super::next_stage,
            )
                .chain(),
        )
        .init_schedule(Fold)
        .add_systems(
            Fold,
            (
                (mark_variables_const, fold_const_declarations),
                fold_variables,
                fold_literal_leaves,
            )
                .chain(),
        );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ScheduleLabel)]
struct Fold;

/// [`Const`] expression.
///
/// Failing to fold is a compilation error.
#[derive(Component)]
struct MustFold;

fn mark_const_expr(mut commands: Commands, consts: Query<&DeclExpr, With<Const>>) {
    for expr in consts.iter() {
        commands.entity(expr.0).insert(MustFold);
    }
}

fn mark_variables_const(
    mut commands: Commands,
    const_decls: Query<(), With<Const>>,
    variables: Query<(Entity, &VariableOf), Without<Const>>,
) {
    for (entity, var_of) in variables.iter() {
        if const_decls.contains(var_of.0) {
            commands.entity(entity).insert(Const);
        }
    }
}

fn verify_variable_const_assign(
    const_variables: Query<&Span, (With<VariableOf>, With<Const>)>,
    bin_ops: Query<&Children, (With<BinOp>, With<Assignment>)>,
) -> Result {
    for children in bin_ops.iter() {
        if let Ok(span) = const_variables.get(children[0]) {
            return Err(span.custom("Can not assign to a constant").into());
        }
    }
    Ok(())
}

#[derive(Default, Resource)]
struct Rerun(bool);

// TODO: If there are errors thrown in here then this gets stuck in an infinite
// loop...
fn fold(world: &mut World) {
    loop {
        world.resource_mut::<Rerun>().0 = false;
        world.run_schedule(Fold);
        if !world.resource::<Rerun>().0 {
            break;
        }
    }
}

#[derive(Message)]
struct Folded {
    entity: Entity,
}

fn fold_const_declarations(
    decls: Query<&DeclExpr, With<Const>>,
    literals: Query<(), With<Literal>>,
    mut writer: MessageWriter<Folded>,
) {
    for expr in decls.iter() {
        if literals.contains(expr.0) {
            writer.write(Folded { entity: expr.0 });
        }
    }
}

fn fold_variables(
    mut commands: Commands,
    variables: Query<(Entity, &VariableOf), With<Const>>,
    decls: Query<(Entity, &DeclExpr), With<Const>>,
    mut folded: MessageReader<Folded>,
    literals: Query<&Literal>,
    mut rerun: ResMut<Rerun>,
) -> Result {
    for folded in folded.read() {
        if let Some(decl) = decls
            .iter()
            .find_map(|(de, d)| (d.0 == folded.entity).then_some(de))
        {
            for (entity, var_of) in variables.iter() {
                if var_of.0 == decl {
                    let literal = literals.get(folded.entity)?;
                    commands
                        .entity(entity)
                        .remove::<(Variable, VariableOf)>()
                        .insert(*literal);
                    rerun.0 = true;
                }
            }
        }
    }
    Ok(())
}

fn fold_literal_leaves(
    mut commands: Commands,
    expr: Query<(Entity, &BinOp, &Children, &Type), (Without<Assignment>, Without<Logical>)>,
    literals: Query<&Literal>,
    mut writer: MessageWriter<Folded>,
    mut rerun: ResMut<Rerun>,
) {
    for (entity, op, children, ty) in expr.iter() {
        if let (Ok(Literal::Integer(lhs)), Ok(Literal::Integer(rhs))) =
            (literals.get(children[0]), literals.get(children[1]))
        {
            let folded = match ty {
                Type::U8 => u8::integer_bin_op(*lhs, *rhs, *op),
                Type::U16 => u16::integer_bin_op(*lhs, *rhs, *op),
                Type::U32 => u32::integer_bin_op(*lhs, *rhs, *op),
                Type::U64 => u64::integer_bin_op(*lhs, *rhs, *op),
                //
                Type::I8 => i8::integer_bin_op(*lhs, *rhs, *op),
                Type::I16 => i16::integer_bin_op(*lhs, *rhs, *op),
                Type::I32 => i32::integer_bin_op(*lhs, *rhs, *op),
                Type::I64 => i64::integer_bin_op(*lhs, *rhs, *op),
                //
                Type::Not | Type::Str => unreachable!(),
            };
            commands
                .entity(entity)
                .remove::<(BinOp, Assignment, Logical)>()
                .insert(Literal::Integer(folded));
            commands.entity(children[0]).despawn();
            commands.entity(children[1]).despawn();
            writer.write(Folded { entity });
            rerun.0 = true;
        }
    }
}

fn verify_folded(exprs: Query<&Span, (With<MustFold>, Without<Literal>)>) -> Result {
    if let Some(span) = exprs.iter().next() {
        return Err(span.custom("Expression is not const").into());
    }
    Ok(())
}

// Run time const evaluation
// I SEE YOU

trait IntegerBinOp: Sized {
    fn integer_bin_op(a: u64, b: u64, op: BinOp) -> u64;
    fn from_u64(v: u64) -> Self;
    fn into_u64(v: Self) -> u64;
}

macro_rules! integer_bin_op {
    ($ty:ident) => {
        impl IntegerBinOp for $ty {
            fn from_u64(v: u64) -> $ty {
                const BYTES: usize = $ty::BITS as usize / 8;
                let mut bytes = [0u8; BYTES];
                bytes.copy_from_slice(&v.to_le_bytes()[..BYTES]);
                $ty::from_le_bytes(bytes)
            }
            fn into_u64(v: $ty) -> u64 {
                const BYTES: usize = $ty::BITS as usize / 8;
                let mut bytes = [0u8; 8];
                bytes[..BYTES].copy_from_slice(&v.to_le_bytes());
                u64::from_le_bytes(bytes)
            }
            fn integer_bin_op(a: u64, b: u64, op: BinOp) -> u64 {
                let lhs = <Self as IntegerBinOp>::from_u64(a);
                let rhs = <Self as IntegerBinOp>::from_u64(b);
                let result = match op {
                    BinOp::Add => lhs.wrapping_add(rhs),
                    BinOp::Sub => lhs.wrapping_sub(rhs),
                    BinOp::Mul => lhs.wrapping_mul(rhs),
                    BinOp::Div => lhs.wrapping_div(rhs),
                    BinOp::Mod => lhs.wrapping_rem(rhs),
                    BinOp::Eq => lhs.eq(&rhs) as $ty,
                    BinOp::Ne => lhs.ne(&rhs) as $ty,
                    BinOp::Gt => lhs.gt(&rhs) as $ty,
                    BinOp::Ge => lhs.ge(&rhs) as $ty,
                    BinOp::Lt => lhs.lt(&rhs) as $ty,
                    BinOp::Le => lhs.le(&rhs) as $ty,
                    BinOp::BitAnd => lhs.bitand(rhs),
                    BinOp::BitOr => lhs.bitor(rhs),
                    BinOp::Xor => lhs.bitxor(rhs),
                    // TODO: I don't have negative literals yet, so this is fine,
                    // but this will have to be checked if they are added!
                    BinOp::Shr => lhs.wrapping_shr(rhs as u32),
                    // TODO: I don't have negative literals yet, so this is fine,
                    // but this will have to be checked if they are added!
                    BinOp::Shl => lhs.wrapping_shl(rhs as u32),
                    _ => unreachable!(),
                };
                <Self as IntegerBinOp>::into_u64(result)
            }
        }
    };
}

integer_bin_op!(u8);
integer_bin_op!(u16);
integer_bin_op!(u32);
integer_bin_op!(u64);
integer_bin_op!(i8);
integer_bin_op!(i16);
integer_bin_op!(i32);
integer_bin_op!(i64);

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn integer_bo() {
        #[track_caller]
        fn check<T: PartialEq + IntegerBinOp>(a: T, b: T, op: BinOp, expected: u64) {
            let a = T::into_u64(a);
            let b = T::into_u64(b);
            let value = T::integer_bin_op(a, b, op);
            assert_eq!(expected, value);
        }
        check(0xfeu8, 1u8, BinOp::Add, 0xff);
        check(0xfeu8, 2u8, BinOp::Add, 0);
        check(0u8, 0u8, BinOp::Add, 0);
        check(5u8, 3u8, BinOp::Sub, 2);
        check(0u8, 1u8, BinOp::Sub, 0xff);
        check(255u8, 255u8, BinOp::Sub, 0);
        check(2u8, 3u8, BinOp::Mul, 6);
        check(200u8, 2u8, BinOp::Mul, 144);
        check(255u8, 1u8, BinOp::Div, 255);
        check(10u8, 3u8, BinOp::Mod, 1);
        check(255u8, 16u8, BinOp::Mod, 15);
        check(1u8, 2u8, BinOp::Eq, 0);
        check(0b1010u8, 0b1100u8, BinOp::BitAnd, 0b1000);
        check(0b1010u8, 0b1100u8, BinOp::BitOr, 0b1110);
        check(0b1010u8, 0b1100u8, BinOp::Xor, 0b0110);
        check(1u8, 3u8, BinOp::Shl, 8);
        check(0x10u8, 2u8, BinOp::Shr, 4);
    }
}
