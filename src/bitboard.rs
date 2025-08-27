use crate::bitboard_console::BitboardConsole;
use std::str::FromStr;

#[derive(Debug)]
pub struct Bitboard {
    pub player_bods: [u64; 2],
    pub piece_bod: u64,
    pub have_piece: [i64; 2],
    pub turn: i64,
    pub turn_player: usize,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Move {
    Place { r: u64, c: u64 },
    Flick { r: u64, c: u64, angle_idx: usize },
}

impl Move {
    pub fn to_string(&self) -> String {
        match self {
            Move::Place { r, c } => format!("S({},{})", r, c),
            Move::Flick { r, c, angle_idx } => format!("F({},{},{})", r, c, angle_idx),
        }
    }
}

pub const BITBOD_WIDTH: u64 = 10;
pub const FIELD_BOD_WIDTH: u64 = 5;
pub const FIELD_BOD_HEIGHT: u64 = 5;
const FIELD_BOD: u64 = {
    const ROW_BIT: u64 = 0b11111;
    let mut result = 0u64;
    let mut r = 0u64;
    while r < FIELD_BOD_HEIGHT {
        result <<= BITBOD_WIDTH;
        result |= ROW_BIT;
        r += 1;
    }
    result
};

const ANGLE: [u64; 4] = [
    1,                //右
    1 + BITBOD_WIDTH, //右下
    BITBOD_WIDTH,     //下
    BITBOD_WIDTH - 1, //左下
];

const ANGLE_LINE: [u64; 8] = {
    let mut result = [0u64; 8];
    let mut count = 0usize;
    // (0,0)を中心に回転させる4パターン
    while count < 4 {
        let mut steps = 0u64;
        while steps < 5 {
            result[count] |= 1u64 << (ANGLE[count] * steps);
            steps += 1;
        }
        count += 1;
    }

    // (4,4)を中心に回転させる4パターン
    let mut count = 4usize;
    let top_corner_bit = 1u64 << (BITBOD_WIDTH * (FIELD_BOD_HEIGHT - 1) + FIELD_BOD_WIDTH - 1);
    while count < 8 {
        let mut steps = 0u64; //target_bit は含まない
        while steps < 5 {
            result[count] |= top_corner_bit >> (ANGLE[count - 4] * steps);
            steps += 1;
        }
        count += 1;
    }
    result
};

impl Bitboard {
    pub fn new(player_bods: [u64; 2], turn: i64) -> Self {
        Self {
            player_bods,
            piece_bod: player_bods[0] | player_bods[1],
            have_piece: [
                5 - player_bods[0].count_ones() as i64,
                5 - player_bods[1].count_ones() as i64,
            ],
            turn,
            turn_player: ((-turn + 1) / 2) as usize,
        }
    }
    pub fn new_initial() -> Self {
        Self {
            player_bods: [0; 2],
            piece_bod: 0,
            have_piece: [5; 2],
            turn: 1,
            turn_player: 0,
        }
    }
    pub fn turn_change(&mut self) {
        self.turn = -self.turn;
        self.turn_player = 1 - self.turn_player;
    }
    pub fn apply_force(&mut self, mv: &Move) {
        match mv {
            &Move::Place { r, c } => {
                self.set_force(r, c);
            }
            &Move::Flick { r, c, angle_idx } => {
                self.flick_force(r, c, angle_idx);
            }
        }
    }
    pub fn set_force(&mut self, r: u64, c: u64) {
        self.player_bods[self.turn_player] |= 1u64 << (r * BITBOD_WIDTH + c);
        self.piece_bod |= 1u64 << (r * BITBOD_WIDTH + c);
        self.have_piece[self.turn_player] -= 1;
        self.turn_change();
    }
    pub fn flick_force(&mut self, r: u64, c: u64, angle_idx: usize) {
        use std::arch::x86_64::{_pdep_u64, _pext_u64};

        let angle = ANGLE[angle_idx % 4];
        let is_positive_angle = angle_idx < 4;
        let mut line = ANGLE_LINE[angle_idx];
        let target_bit = 1u64 << (BITBOD_WIDTH * r + c);

        if is_positive_angle {
            //左シフトで表す方向
            //駒の場所にlineの先端を移動する
            line <<= BITBOD_WIDTH * r + c;
            line &= FIELD_BOD; //5*5に収まるようにマスク
            let mut line_piece = self.piece_bod & line;

            //各駒のうちの駒種類の振り分けを記憶
            let piece_order: u64 = unsafe { _pext_u64(self.player_bods[0], line_piece) };

            //piece_bodとplayer_bodsの中のlineに被るところを消す
            self.piece_bod &= !line;
            self.player_bods[0] &= !line;
            self.player_bods[1] &= !line;

            //弾く操作を実行
            line_piece ^= target_bit; //target_bitを消す。
            line_piece >>= angle;
            line_piece |= line & !(line >> angle); //lineの最上位のbitを取得し追加

            //再配置
            self.piece_bod |= line_piece;
            unsafe {
                self.player_bods[0] |= _pdep_u64(piece_order, line_piece);
                self.player_bods[1] |= _pdep_u64(!piece_order, line_piece);
            }
        } else {
            //右シフトで表す方向
            //駒の場所にlineの先端を移動する
            line >>= BITBOD_WIDTH * (FIELD_BOD_HEIGHT - 1 - r) + (FIELD_BOD_WIDTH - 1 - c);
            line &= FIELD_BOD; //5*5に収まるようにマスク
            let mut line_piece = self.piece_bod & line;

            //各駒のうちの駒種類の振り分けを記憶
            let piece_order: u64 = unsafe { _pext_u64(self.player_bods[0], line_piece) };

            //piece_bodとplayer_bodsの中のlineに被るところを消す
            self.piece_bod &= !line;
            self.player_bods[0] &= !line;
            self.player_bods[1] &= !line;

            //弾く操作を実行
            line_piece ^= target_bit; //target_bitを消す
            line_piece <<= angle;
            line_piece |= line & line.wrapping_neg(); //lineの最下位のbitを取得し追加

            //再配置
            self.piece_bod |= line_piece;
            unsafe {
                self.player_bods[0] |= _pdep_u64(piece_order, self.piece_bod);
                self.player_bods[1] |= _pdep_u64(piece_order, self.piece_bod);
            }
        }
        self.turn_change();
    }
}
