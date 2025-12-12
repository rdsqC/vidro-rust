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

        let offset_pp = 0;
        let p1_iter = BitIter(p1_packed);
        let p2_iter = BitIter(p2_packed).map(|idx| idx + NUM_VALID_SQUARES);

        let p1p2_iter = p1_iter.chain(p2_iter.clone());

        // 0~324 len=325
        let pp_iter = p1p2_iter.clone().flat_map(move |sq1| {
            p1p2_iter
                .clone()
                .filter(move |&sq2| sq1 <= sq2)
                .map(move |sq2| offset_pp + (sq1 * NUM_VALID_SQUARES + sq2))
        });

        let turn_offset = NUM_VALID_SQUARES * 2;
        //0~0 len=1
        let turn_iter = if self.turn == 0 {
            Some(turn_offset).into_iter()
        } else {
            None.into_iter()
        };

        //bias_iter0~0 len=1
        let bias_iter = Some(turn_offset + 1).into_iter();

        let hand_piece_offset = turn_offset + 2;

        //bias_iter and hand_piece_iter 0~11 len=12
        let hand_piece_iter = [
            hand_piece_offset + self.p1_hand_piece as usize,
            hand_piece_offset + 6 + self.p2_hand_piece as usize,
        ]
        .into_iter();

        let prev_move_offset = hand_piece_offset + 12;
        //Set 0 ~ 24, Flick(Shoot) 25 ~ 224 len=225
        let prev_move_iter = self
            .prev_move
            .map(|mv| {
                if mv.angle_idx < 8 {
                    prev_move_offset
                        + NUM_VALID_SQUARES
                        + mv.field_idx() as usize
                        + NUM_VALID_SQUARES * mv.angle_idx as usize
                } else {
                    prev_move_offset + mv.field_idx() as usize
                }
            })
            .into_iter();

        pp_iter
            .chain(turn_iter)
            .chain(bias_iter)
            .chain(hand_piece_iter)
            .chain(prev_move_iter)
    }
}
