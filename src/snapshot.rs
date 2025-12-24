#[derive(Clone, Copy, Debug)]
pub struct BoardSnapshot {
    pub p1: u64,
    pub p2: u64,
    pub turn: i8, // 1が先手, -1が後手
    pub p1_hand_piece: u8,
    pub p2_hand_piece: u8,
    pub prev_hash: Option<u64>,
}

impl BoardSnapshot {
    //先手目線変換
    pub fn to_relative(&self) -> Self {
        if self.turn == 1 {
            *self
        } else {
            Self {
                p1: self.p2,
                p2: self.p1,
                turn: 1,
                p1_hand_piece: self.p2_hand_piece,
                p2_hand_piece: self.p1_hand_piece,

                ..*self
            }
        }
    }
}
