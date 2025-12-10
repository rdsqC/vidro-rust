use crate::bitboard::{BITBOD_WIDTH, FIELD_BOD, FIELD_BOD_WIDTH, MoveBit};
use crate::snapshot::BoardSnapshot;
use std::arch::x86_64::_pext_u64;

const NUM_VALID_SQUARES: usize = FIELD_BOD.count_ones() as usize;
pub const NUM_FEATURES: usize = 289;

#[derive(Clone, Copy)]
pub struct BitIter(u64);

impl Iterator for BitIter {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 0 {
            return None;
        }

        let index = self.0.trailing_zeros();
        self.0 &= self.0 - 1;
        Some(index as usize)
    }
}

pub trait BoardSnapshotFeatures {
    fn iter_feature_indices(&self) -> impl Iterator<Item = usize> + '_;
}

impl BoardSnapshotFeatures for BoardSnapshot {
    fn iter_feature_indices(&self) -> impl Iterator<Item = usize> + '_ {
        let p1_packed = unsafe { _pext_u64(self.p1, FIELD_BOD) };
        let p2_packed = unsafe { _pext_u64(self.p2, FIELD_BOD) };

        // 0~49
        let p1_iter = BitIter(p1_packed);
        let p2_iter = BitIter(p2_packed).map(|idx| idx + NUM_VALID_SQUARES);

        let turn_offset = NUM_VALID_SQUARES * 2;
        //0~0
        let turn_iter = if self.turn == 0 {
            Some(turn_offset).into_iter()
        } else {
            None.into_iter()
        };

        //bias_iter0~0
        let bias_iter = Some(turn_offset + 1).into_iter();

        let hand_piece_offset = turn_offset + 2;

        //bias_iter and hand_piece_iter 0~11
        let hand_piece_iter = [
            hand_piece_offset + self.p1_hand_piece as usize,
            hand_piece_offset + 6 + self.p2_hand_piece as usize,
        ]
        .into_iter();

        let prev_move_offset = hand_piece_offset + 12;
        //Set 0 ~ 24, Flick(Shoot) 25 ~ 224
        let prev_move_iter = self
            .prev_move
            .map(|mv| {
                let mv_idx = mv.idx as usize % BITBOD_WIDTH as usize
                    + mv.idx as usize / BITBOD_WIDTH as usize * FIELD_BOD_WIDTH as usize;
                if mv.angle_idx < 8 {
                    prev_move_offset
                        + NUM_VALID_SQUARES
                        + mv_idx as usize
                        + NUM_VALID_SQUARES * mv.angle_idx as usize
                } else {
                    prev_move_offset + mv_idx as usize
                }
            })
            .into_iter();

        p1_iter
            .chain(p2_iter)
            .chain(turn_iter)
            .chain(bias_iter)
            .chain(hand_piece_iter)
            .chain(prev_move_iter)
    }
}
