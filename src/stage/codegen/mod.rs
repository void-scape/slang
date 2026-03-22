use crate::stage::ir::*;
use crate::tree::*;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

mod asm;

pub fn plugin(app: &mut App) {
    app.add_plugins(asm::plugin);
}

#[derive(Component)]
pub struct Prog(pub Vec<Ir>);

fn collect_ir_into_prog(
    mut commands: Commands,
    procs: Query<&Irs, With<Proc>>,
    irs: Query<&Irs>,
    ir_of: Query<(Entity, Option<&IrSub>, Option<&Ir>)>,
) {
    fn ir_leaves(
        prog: &mut Vec<Ir>,
        entity: Entity,
        ir_of: &Query<(Entity, Option<&IrSub>, Option<&Ir>)>,
        irs: &Query<&Irs>,
    ) {
        if let Ok((_, maybe_sub, maybe_ir)) = ir_of.get(entity) {
            if let Some(IrSub(sub_entity)) = maybe_sub {
                if let Ok(sub) = irs.get(*sub_entity) {
                    for child in sub.iter() {
                        ir_leaves(prog, child, ir_of, irs);
                    }
                }
            } else if let Some(ir) = maybe_ir {
                prog.push(ir.clone());
            }
        }
    }

    let mut prog = Vec::new();
    for proc in procs.iter() {
        for child in proc.iter() {
            ir_leaves(&mut prog, child, &ir_of, &irs);
        }
    }
    commands.spawn(Prog(prog));
}
