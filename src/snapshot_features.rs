use crate::bitboard::{BITBOD_WIDTH, FIELD_BOD, FIELD_BOD_WIDTH};
use crate::random_state_generator::random_state_generator;
use crate::snapshot::BoardSnapshot;
use std::arch::x86_64::{_pdep_u64, _pext_u64};

macro_rules! build_features {
    ($snapshot: expr, [  $($feature_type:ty),* ]) => {
        {
            build_features!(@recurse, $snapshot, 0, $($feature_type),*)
        }
    };

    (@recurse, $snapshot: expr, $current_offset: expr, $head: ty, $($rest:ty),+) => {
        <$head as FeatureGroup>::get_iter($snapshot, $current_offset)
            .chain(
                build_features!(
                    @recurse,
                    $snapshot,
                    ($current_offset + <$head as FeatureGroup>::LEN),
                    $($rest),+
                )
            )
    };

    (@recurse, $snapshot:expr, $current_offset:expr, $last:ty) => {
        <$last as FeatureGroup>::get_iter($snapshot, $current_offset)
    };
}

macro_rules! count_total_features {
    ($($feature_type:ty),*) => {
        0 $( + <$feature_type as FeatureGroup>::LEN )*
    };
}

trait FeatureGroup {
    const LEN: usize;

    fn get_iter(snapshot: BoardSnapshot, offset_set: usize) -> impl Iterator<Item = usize>;
}

macro_rules! define_ai_model {
    (
        target: $snapshot_type:ty,
        features: [$($feature:ty),* $(,)?]
     ) => {
        pub const NUM_FEATURES: usize = count_total_features!($($feature),*);

        impl BoardSnapshotFeatures for $snapshot_type {
            fn iter_feature_indices(&self) -> impl Iterator<Item = usize> + '_ {
                let relative_snapshot = self.to_relative();
                return build_features!(relative_snapshot, [$($feature),*]);
            }
        }
    };
}

macro_rules! run_feature_tests {
    ($($feature_type:ty),*) => {
        for _ in 0..10000 {
            let (board, _) = random_state_generator(16);
            let snapshot = board.to_snapshot(None);
            board.print_data();
            $(
                let def_len: usize = <$feature_type as FeatureGroup>::LEN;
                let count: usize = <$feature_type as FeatureGroup>::get_iter(snapshot, 0).max().unwrap_or(0);

                assert!(count <= def_len, "Feature length {} has is {}. But there are featrue which has {} offset", stringify!($features_type), def_len, count);
            )*
        }
    };
}

define_ai_model!(
    target: BoardSnapshot,
    features: [
        LineFeatures,
        HandPieceFeatures,
        PrevPPFeatures,
        PrevLineFeatures,
        PrevHandPieceFeatures,
        PPFeatures,
        PPPFeature,
    ]
);

#[test]
fn test() {
    use crate::bitboard_console::BitboardConsole;

    run_feature_tests!(
        LineFeatures,
        HandPieceFeatures,
        PrevPPFeatures,
        PrevLineFeatures,
        PrevHandPieceFeatures,
        PPFeatures,
        PPPFeature
    );
}

const NUM_VALID_SQUARES: usize = FIELD_BOD.count_ones() as usize;

#[derive(Clone, Copy)]
pub struct BitIter(pub u64);

impl BitIter {
    pub fn new(init_value: u64) -> Self {
        Self(init_value)
    }
}

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

#[derive(Clone, Copy)]
pub struct BitMapIter(u64);

impl Iterator for BitMapIter {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 0 {
            return None;
        }

        let index = self.0.trailing_zeros();

        let field_idx = index as usize / BITBOD_WIDTH as usize * FIELD_BOD_WIDTH as usize
            + index as usize % BITBOD_WIDTH as usize;

        self.0 &= self.0 - 1;
        Some(field_idx as usize)
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

const NUM_LINE3: usize = 4;
const NUM_LINE4: usize = 4;
const NUM_LINE5: usize = 20;
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

    //sort ascending order
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

const fn const_pext(src: u64, mask: u64) -> u64 {
    let mut res = 0;
    let mut dest_bit = 0;
    let mut i = 0;

    while i < 64 {
        let bit = 1 << i;
        if (mask & bit) != 0 {
            if (src & bit) != 0 {
                res |= 1 << dest_bit;
            }
            dest_bit += 1;
        }
        i += 1;
    }
    res
}

const FEATURE_LINES_FOR_PREV: [LineConfig; 20] = {
    let mut result = [ZERO_CONFIG; FEATURE_LINES.len()];
    let mut count = 0;
    while count < FEATURE_LINES.len() {
        result[count] = LineConfig {
            mask: const_pext(FEATURE_LINES[count].mask, FIELD_BOD),
            length: FEATURE_LINES[count].length,
            offset: FEATURE_LINES[count].offset,
        };
        count += 1;
    }
    result
};

pub struct LineConfig {
    pub mask: u64,
    pub length: usize,
    pub offset: usize,
}

const MAX_LINE_LEN: usize = 5;
const TABLE_SIZE: usize = 1 << (MAX_LINE_LEN * 2);

static TERNARY_LUT: [u16; TABLE_SIZE] = generate_ternary_table();

const fn generate_ternary_table() -> [u16; TABLE_SIZE] {
    let mut table = [0; TABLE_SIZE];
    let mut i = 0;

    while i < TABLE_SIZE {
        let a = (i as u64) & ((1 << MAX_LINE_LEN) - 1);
        let b = (i as u64) >> MAX_LINE_LEN;

        if (a & b) != 0 {
            table[i] = 0;
        } else {
            let mut val = 0;
            let mut weight = 1;
            let mut bit = 0;

            while bit < MAX_LINE_LEN {
                let bit_a = ((a >> bit) & 1) as u16;
                let bit_b = ((b >> bit) & 1) as u16;

                val += (bit_a * bit_b * 2) * weight;
                weight *= 3;
                bit += 1;
            }
            table[i] = val;
        }
        i += 1;
    }
    table
}

#[inline(always)]
fn encode_ternary_lut(a: u64, b: u64) -> usize {
    debug_assert!((a & b) == 0, "a and b must not overlap");

    let index = (a | (b << MAX_LINE_LEN)) as usize;

    TERNARY_LUT[index] as usize
}

#[inline(always)]
const fn combination(n: usize, r: usize) -> usize {
    if n < r {
        return 0;
    }
    let mut result: usize = 1;

    let mut count = 0;
    while count < r {
        result *= n - count;
        count += 1;
    }

    let mut per = 1;
    let mut count = 0;
    while count < r {
        per *= 1 + count;
        count += 1;
    }

    result /= per;

    result
}

//2駒関係
struct PPFeatures;

impl FeatureGroup for PPFeatures {
    const LEN: usize = combination(25, 2) * 2usize.pow(2);
    fn get_iter(snapshot: BoardSnapshot, offset_set: usize) -> impl Iterator<Item = usize> {
        let p1_packed = unsafe { _pext_u64(snapshot.p1, FIELD_BOD) };
        let p2_packed = unsafe { _pext_u64(snapshot.p2, FIELD_BOD) };

        let p1_iter = BitIter(p1_packed).map(|sq| (sq, 0usize));
        let p2_iter = BitIter(p2_packed).map(|sq| (sq, 1usize));
        let p1p2_iter = p1_iter.chain(p2_iter);

        let pp_iter = p1p2_iter.clone().flat_map(move |(sq1, sq1_player)| {
            p1p2_iter
                .clone()
                .filter(move |&(sq2, _)| sq1 < sq2)
                .map(move |(sq2, sq2_player)| {
                    let spatial_idx = (combination(sq1, 2)) + combination(sq2, 1);

                    let type_idx = (sq1_player << 1) | sq2_player;

                    offset_set + spatial_idx + combination(25, 2) * type_idx
                })
        });

        pp_iter
    }
}

//3駒関係
struct PPPFeature;

impl FeatureGroup for PPPFeature {
    const LEN: usize = combination(25, 3) * 2usize.pow(3);
    fn get_iter(snapshot: BoardSnapshot, offset_set: usize) -> impl Iterator<Item = usize> {
        let p1_packed = unsafe { _pext_u64(snapshot.p1, FIELD_BOD) };
        let p2_packed = unsafe { _pext_u64(snapshot.p2, FIELD_BOD) };

        let occupied = p1_packed | p2_packed;

        BitIter(occupied).flat_map(move |sq1| {
            let mask2 = occupied & !((1u64 << (sq1 + 1)) - 1);
            BitIter(mask2).flat_map(move |sq2| {
                let mask3 = occupied & !((1u64 << (sq2 + 1)) - 1);
                BitIter(mask3).map(move |sq3| {
                    let pl1 = if (p1_packed >> sq1) & 1 == 1 { 0 } else { 1 };
                    let pl2 = if (p1_packed >> sq2) & 1 == 1 { 0 } else { 1 };
                    let pl3 = if (p1_packed >> sq3) & 1 == 1 { 0 } else { 1 };

                    let spatial_idx =
                        combination(sq3, 3) + combination(sq2, 2) + combination(sq1, 1);

                    let type_idx = (pl1 << 2) | (pl2 << 1) | pl3;

                    offset_set + combination(25, 3) * type_idx + spatial_idx
                })
            })
        })
    }
}

//LINE
struct LineFeatures;

impl FeatureGroup for LineFeatures {
    const LEN: usize = 3348;
    fn get_iter(snapshot: BoardSnapshot, offset_set: usize) -> impl Iterator<Item = usize> {
        FEATURE_LINES.iter().map(move |line_config| {
            let white_line: u64 = unsafe { _pext_u64(snapshot.p1, line_config.mask) };
            let black_line: u64 = unsafe { _pext_u64(snapshot.p2, line_config.mask) };
            offset_set + line_config.offset + encode_ternary_lut(white_line, black_line)
        })
    }
}

//Bias
struct BiasFeatures;

impl FeatureGroup for BiasFeatures {
    const LEN: usize = 1;
    fn get_iter(_: BoardSnapshot, offset_set: usize) -> impl Iterator<Item = usize> {
        Some(offset_set).into_iter()
    }
}

//Turn
struct TurnFeatures;

impl FeatureGroup for TurnFeatures {
    const LEN: usize = 1;
    fn get_iter(snapshot: BoardSnapshot, offset_set: usize) -> impl Iterator<Item = usize> {
        if snapshot.turn == 1 {
            Some(offset_set).into_iter()
        } else {
            None.into_iter()
        }
    }
}

//HandPiece
struct HandPieceFeatures;

impl FeatureGroup for HandPieceFeatures {
    const LEN: usize = 12;
    fn get_iter(snapshot: BoardSnapshot, offset_set: usize) -> impl Iterator<Item = usize> {
        [
            offset_set + snapshot.p1_hand_piece as usize,
            offset_set + snapshot.p2_hand_piece as usize + 6,
        ]
        .into_iter()
    }
}

//PrevPP
struct PrevPPFeatures;

impl FeatureGroup for PrevPPFeatures {
    const LEN: usize = (25 + 1) * 25 / 2;
    fn get_iter(snapshot: BoardSnapshot, offset_set: usize) -> impl Iterator<Item = usize> {
        snapshot.prev_hash.into_iter().flat_map(move |hash| {
            let p1_iter = BitIter(snapshot.prev_hash.unwrap() >> (NUM_VALID_SQUARES + 1));
            let p2_iter =
                BitIter(((1 << NUM_VALID_SQUARES) - 1) & (snapshot.prev_hash.unwrap() >> 1))
                    .map(|idx| idx + NUM_VALID_SQUARES);
            let p1p2_iter = p1_iter.chain(p2_iter);

            let pp_iter = p1p2_iter.clone().flat_map(move |sq1| {
                p1p2_iter
                    .clone()
                    .filter(move |&sq2| sq1 <= sq2)
                    .map(move |sq2| offset_set + (sq1 * NUM_VALID_SQUARES + sq2))
            });

            pp_iter
        })
    }
}

//PrevLINE
struct PrevLineFeatures;

impl FeatureGroup for PrevLineFeatures {
    const LEN: usize = 3348;
    fn get_iter(snapshot: BoardSnapshot, offset_set: usize) -> impl Iterator<Item = usize> {
        snapshot.prev_hash.into_iter().flat_map(move |hash| {
            let p1 = snapshot.prev_hash.unwrap() >> (NUM_VALID_SQUARES + 1);
            let p2 = ((1 << NUM_VALID_SQUARES) - 1) & (snapshot.prev_hash.unwrap() >> 1);
            FEATURE_LINES_FOR_PREV.iter().map(move |line_config| {
                let white_line: u64 = unsafe { _pext_u64(p1, line_config.mask) };
                let black_line: u64 = unsafe { _pext_u64(p2, line_config.mask) };
                offset_set + line_config.offset + encode_ternary_lut(white_line, black_line)
            })
        })
    }
}

//PrevHandPiece
struct PrevHandPieceFeatures;

impl FeatureGroup for PrevHandPieceFeatures {
    const LEN: usize = 12;
    fn get_iter(snapshot: BoardSnapshot, offset_set: usize) -> impl Iterator<Item = usize> {
        snapshot.prev_hash.into_iter().flat_map(move |hash| {
            [
                offset_set + hash.count_ones() as usize,
                offset_set + hash.count_ones() as usize + 6,
            ]
            .into_iter()
        })
    }
}
