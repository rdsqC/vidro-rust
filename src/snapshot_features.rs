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

const ZERO_CONFIG: LineConfig = LineConfig {
    mask: 0,
    length: 0,
    offset: 0,
};

pub const FEATURE_LINES: [LineConfig; 20] = {
    let mut masks = [0u64; 20];

    let mut vertical_line = 0u64;
    let mut i = 0;
    while i < 5 {
        vertical_line |= 0b000001 << BITBOD_WIDTH * i;
        i += 1;
    }

    let mut i = 0;
    let mut sq_idx = 0;
    while i < 5 {
        masks[sq_idx] = 0b11111 << BITBOD_WIDTH * i as u64;
        i += 1;
        sq_idx += 1;
    }

    let mut i = 0;
    while i < 5 {
        masks[sq_idx] = vertical_line << i;
        i += 1;
        sq_idx += 1;
    }

    let right_diagonal_line = {
        let mut masks = 0u64;
        let mut count = 0;
        while count < 5 {
            masks |= 0b10000 << count * (BITBOD_WIDTH - 1);
            count += 1;
        }
        masks
    };
    let mut i = 0;
    while i < 2 {
        masks[sq_idx] = FIELD_BOD & (right_diagonal_line >> (2 - i));
        i += 1;
        sq_idx += 1;
    }
    let mut i = 0;
    while i < 3 {
        masks[sq_idx] = FIELD_BOD & (right_diagonal_line << i);
        i += 1;
        sq_idx += 1;
    }

    let left_diagonal_line = {
        let mut masks = 0u64;
        let mut count = 0;
        while count < 5 {
            masks |= 0b00001 << count * (BITBOD_WIDTH + 1);
            count += 1;
        }
        masks
    };
    let mut i = 0;
    while i < 2 {
        masks[sq_idx] = FIELD_BOD & (left_diagonal_line >> (2 - i));
        i += 1;
        sq_idx += 1;
    }
    let mut i = 0;
    while i < 3 {
        masks[sq_idx] = FIELD_BOD & (left_diagonal_line << i);
        i += 1;
        sq_idx += 1;
    }

    let mut i = 0;
    while i < 20 {
        let mut j = 0;
        while j < 20 - 1 - i {
            if masks[i].count_ones() > masks[j + 1].count_ones() {
                let temp = masks[j];
                masks[j] = masks[j + 1];
                masks[j + 1] = temp;
            }
            j += 1;
        }
        i += 1;
    }

    let mut configs = [ZERO_CONFIG; 20];
    let mut current_offset = 0;

    let mut k = 0;
    while k < 20 {
        let m = masks[k];
        let len = m.count_ones() as usize;
        let num_features = 3usize.pow(len as u32);

        configs[k] = LineConfig {
            mask: m,
            length: len,
            offset: current_offset,
        };

        current_offset += num_features;
        k += 1;
    }

    configs
};

struct LineConfig {
    mask: u64,
    length: usize,
    offset: usize,
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
