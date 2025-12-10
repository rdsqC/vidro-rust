use crate::bitboard::MoveBit;

#[derive(Clone, Copy)]
pub struct BoardSnapshot {
    pub p1: u64,
    pub p2: u64,
    pub turn: i8, // 1が先手, -1が後手
    pub p1_hand_piece: u8,
    pub p2_hand_piece: u8,
    pub prev_move: Option<MoveBit>,
}
