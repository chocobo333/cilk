use crate::codegen::common::machine::{function::MachineFunction, module::MachineModule};
use crate::traits::basic_block::BasicBlocksTrait;
use crate::traits::pass::ModulePassTrait;

// Must run after phi elimination
pub struct BranchFolding {}

impl ModulePassTrait for BranchFolding {
    type M = MachineModule;

    fn name(&self) -> &'static str {
        "BranchFolding"
    }

    fn run_on_module(&mut self, module: &mut Self::M) {
        self.run_on_module(module)
    }
}

impl BranchFolding {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run_on_module(&mut self, module: &mut MachineModule) {
        for (_, f) in &mut module.functions {
            if f.is_internal {
                continue;
            }
            self.remove_empty_block(f);
            self.merge_blocks(f);
        }
    }

    fn merge_blocks(&mut self, f: &mut MachineFunction) {
        loop {
            let mut blocks_to_merge = vec![];
            for (id, block) in f.body.basic_blocks.id_and_block() {
                if block.succ.len() == 0 || block.pred.len() == 0 {
                    continue;
                }

                let mergeable_into_succ = (block.succ.len() == 1
                    || (block.succ.len() > 1 && block.succ.windows(2).all(|s| s[0] == s[1])))
                    && {
                        let succ_preds = &f.body.basic_blocks.get_arena()[block.succ[0]].pred;
                        (succ_preds.len() == 1 && succ_preds[0] == id)
                            || (succ_preds.len() > 1 && succ_preds.iter().all(|s| s == &id))
                    };

                if mergeable_into_succ {
                    blocks_to_merge.push(id);
                    break;
                }
            }
            if blocks_to_merge.len() == 0 {
                break;
            }
            for block in blocks_to_merge {
                let block_ = &f.body.basic_blocks.get_arena()[block];
                let mut remove = vec![];
                for &inst_id in block_.iseq_ref().iter().rev() {
                    if !f.body.inst_arena[inst_id].opcode.is_terminator() {
                        break;
                    }
                    remove.push(inst_id);
                }
                for id in remove {
                    f.remove_inst(id);
                }
                let succ = f.body.basic_blocks.get_arena()[block].succ[0];
                for &inst_id in &*f.body.basic_blocks.get_arena()[succ].iseq_ref() {
                    f.body.inst_arena[inst_id].parent = block;
                }
                // merge succ into block
                f.body.basic_blocks.merge(&block, &succ);
            }
        }
    }

    // Very simple branch folding. TODO: Implement further complicated one
    fn remove_empty_block(&mut self, f: &mut MachineFunction) {
        let mut worklist = vec![];

        for (id, block) in f.body.basic_blocks.id_and_block() {
            if block.iseq_ref().len() > 1 {
                continue;
            }
            let inst = &f.body.inst_arena[block.iseq_ref()[0]];
            if inst.opcode.is_unconditional_jmp() {
                worklist.push((id, inst.operand[0].as_basic_block()));
            }
        }

        for &(block_to_remove, new_dst) in &worklist {
            let preds = f.body.basic_blocks.arena[block_to_remove].pred.clone();
            let succs = f.body.basic_blocks.arena[block_to_remove].succ.clone();

            for &bb in &preds {
                let cur = &mut f.body.basic_blocks.arena[bb];
                cur.succ.remove_item(&block_to_remove).unwrap();
                cur.succ.push(new_dst);
            }

            for &bb in &succs {
                let cur = &mut f.body.basic_blocks.arena[bb];
                cur.pred.remove_item(&block_to_remove).unwrap();
                cur.pred.append(&mut preds.clone());
            }

            for &bb in &preds {
                let cur = &mut f.body.basic_blocks.arena[bb];
                for inst_id in cur.iseq_ref().iter().rev() {
                    let inst = &mut f.body.inst_arena[*inst_id];
                    if !inst.opcode.is_terminator() {
                        break;
                    }
                    inst.replace_operand_block(block_to_remove, new_dst);
                }
            }
        }

        debug!(println!("{} blocks removed", worklist.len()));

        for (remove, _) in worklist {
            f.body.basic_blocks.order.remove_item(&remove).unwrap();
        }
    }
}
