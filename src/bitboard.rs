use crate::bitboard_console::BitboardConsole;
use std::str::FromStr;

#[derive(Debug)]
pub struct Bitboard {
    pub player_bods: [u64; 2],
    pub have_piece: [u8; 2],
    pub turn: i8,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MoveBit {
    idx: u8,
    angle_idx: u8, //8以上のときはset
}

impl MoveBit {
    pub fn to_string(&self) -> String {
        let r = self.angle_idx / 5;
        let c = self.angle_idx % 5;
        if self.angle_idx < 8 {
            //flick
            format!("F({},{},{})", r, c, self.angle_idx)
        } else {
            format!("S({},{})", r, c)
        }
    }
    pub fn new(r: u8, c: u8, idx: u8) -> MoveBit {
        MoveBit {
            idx: r * BITBOD_WIDTH as u8 + c,
            angle_idx: idx,
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
    pub fn new(player_bods: [u64; 2], turn: i8) -> Self {
        Self {
            player_bods,
            have_piece: [
                5 - player_bods[0].count_ones() as u8,
                5 - player_bods[1].count_ones() as u8,
            ],
            turn,
            // turn_player: ((-turn + 1) / 2) as u8,
        }
    }
    pub fn new_initial() -> Self {
        Self {
            player_bods: [0; 2],
            have_piece: [5; 2],
            turn: 1,
        }
    }
    pub fn turn_change(&mut self) {
        self.turn = -self.turn;
    }
    pub fn apply_force(&mut self, mv: MoveBit) {
        if mv.angle_idx < 8 {
            self.flick_force(mv);
        } else {
            self.set_force(mv);
        }
    }
    pub fn undo_force(&mut self, mv: MoveBit) {
        if mv.angle_idx < 8 {
            self.flick_undo_force(mv);
        } else {
            self.set_undo_force(mv);
        }
    }
    pub fn set_force(&mut self, mv: MoveBit) {
        let turn_player = ((-self.turn + 1) / 2) as usize;
        let target_bit = 1u64 << mv.idx;
        debug_assert!(
            (1u64 << mv.idx | (self.player_bods[0] | self.player_bods[1]))
                != self.player_bods[0] | self.player_bods[1],
            "target_bit is within the bit sequence piece_bod"
        );
        self.player_bods[turn_player] |= target_bit;
        self.have_piece[turn_player] -= 1;
        self.turn_change();
        debug_assert!(
            self.player_bods[0] | FIELD_BOD == FIELD_BOD,
            "player_bods[0] is protrude beyond FIELD_BOD"
        );
        debug_assert!(
            self.player_bods[1] | FIELD_BOD == FIELD_BOD,
            "player_bods[1] is protrude beyond FIELD_BOD"
        );
    }
    pub fn flick_force(&mut self, mv: MoveBit) {
        use std::arch::x86_64::{_pdep_u64, _pext_u64};

        let angle = ANGLE[mv.angle_idx as usize % 4];
        let is_positive_angle = mv.angle_idx < 4;
        let mut line = ANGLE_LINE[mv.angle_idx as usize];
        let target_bit = 1u64 << mv.idx;

        debug_assert!(
            target_bit & (self.player_bods[0] | self.player_bods[1]) == target_bit,
            "target_bit is protrude beyand piece_bod"
        );

        if is_positive_angle {
            //左シフトで表す方向
            //駒の場所にlineの先端を移動する
            line <<= mv.idx;

            line &= FIELD_BOD; //5*5に収まるようにマスク
            let mut line_piece = (self.player_bods[0] | self.player_bods[1]) & line;

            //各駒のうちの駒種類の振り分けを記憶
            let piece_order: u64 = unsafe { _pext_u64(self.player_bods[0], line_piece) };

            //piece_bodとplayer_bodsの中のlineに被るところを消す
            self.player_bods[0] &= !line;
            self.player_bods[1] &= !line;

            //弾く操作を実行
            line_piece ^= target_bit; //target_bitを消す。
            line_piece >>= angle;
            line_piece |= line & !(line >> angle); //lineの最上位のbitを取得し追加

            //再配置
            unsafe {
                self.player_bods[0] |= _pdep_u64(piece_order, line_piece);
                self.player_bods[1] |= _pdep_u64(!piece_order, line_piece);
            }
        } else {
            //右シフトで表す方向
            //駒の場所にlineの先端を移動する
            line >>= (BITBOD_WIDTH * (FIELD_BOD_HEIGHT - 1) + FIELD_BOD_WIDTH - 1) as u8 - mv.idx;
            line &= FIELD_BOD; //5*5に収まるようにマスク
            let mut line_piece = (self.player_bods[0] | self.player_bods[1]) & line;

            //各駒のうちの駒種類の振り分けを記憶
            let piece_order: u64 = unsafe { _pext_u64(self.player_bods[0], line_piece) };

            //piece_bodとplayer_bodsの中のlineに被るところを消す
            self.player_bods[0] &= !line;
            self.player_bods[1] &= !line;

            //弾く操作を実行
            line_piece ^= target_bit; //target_bitを消す
            line_piece <<= angle;
            line_piece |= line & line.wrapping_neg(); //lineの最下位のbitを取得し追加

            //再配置
            unsafe {
                self.player_bods[0] |= _pdep_u64(piece_order, line_piece);
                self.player_bods[1] |= _pdep_u64(!piece_order, line_piece);
            }
        }
        debug_assert!(
            self.player_bods[0] & self.player_bods[1] == 0,
            "bod of first player and bod of second player overlap"
        );
        debug_assert!(
            self.player_bods[0] | FIELD_BOD == FIELD_BOD,
            "player_bods[0] is protrude beyond FIELD_BOD"
        );
        debug_assert!(
            self.player_bods[1] | FIELD_BOD == FIELD_BOD,
            "player_bods[1] is protrude beyond FIELD_BOD"
        );
        self.turn_change();
    }
    pub fn set_undo_force(&mut self, mv: MoveBit) {
        debug_assert!(
            (1u64 << mv.idx | (self.player_bods[0] | self.player_bods[1]))
                == (self.player_bods[0] | self.player_bods[1]),
            "target_bit is not within the bit sequence piece_bod"
        );
        self.turn_change();
        let turn_player = ((-self.turn + 1) / 2) as usize;
        let target_bit = 1u64 << mv.idx;
        self.player_bods[turn_player] &= !target_bit;
        self.have_piece[turn_player] += 1;
        debug_assert!(
            self.player_bods[0] | FIELD_BOD == FIELD_BOD,
            "player_bods[0] is protrude beyond FIELD_BOD"
        );
        debug_assert!(
            self.player_bods[1] | FIELD_BOD == FIELD_BOD,
            "player_bods[1] is protrude beyond FIELD_BOD"
        );
    }
    pub fn flick_undo_force(&mut self, mv: MoveBit) {
        self.turn_change();
        use std::arch::x86_64::{_pdep_u64, _pext_u64};

        let angle = ANGLE[mv.angle_idx as usize % 4];
        let is_positive_angle = mv.angle_idx < 4;
        let mut line = ANGLE_LINE[mv.angle_idx as usize];
        let target_bit = 1u64 << mv.idx;
        if is_positive_angle {
            //左シフトで表す方向
            //駒の場所にlineの先端を移動する
            line <<= mv.idx;

            line &= FIELD_BOD; //5*5に収まるようにマスク
            let mut line_piece = (self.player_bods[0] | self.player_bods[1]) & line;

            //再配置を取り消す
            let piece_order = unsafe { _pext_u64(self.player_bods[0], line_piece) }; //順序記憶
            self.player_bods[0] &= !line;
            self.player_bods[1] &= !line;

            //弾く操作の逆を実行
            line_piece &= !(line & !(line >> angle)); //lineの最上位のbitを取得し削除
            line_piece <<= angle;
            line_piece |= target_bit; //target_bitを追加

            //再配置
            unsafe {
                self.player_bods[0] |= _pdep_u64(piece_order, line_piece);
                self.player_bods[1] |= _pdep_u64(!piece_order, line_piece);
            }
        } else {
            //右シフトで表す方向
            //駒の場所にlineの先端を移動する
            line >>= (BITBOD_WIDTH * (FIELD_BOD_HEIGHT - 1) + FIELD_BOD_WIDTH - 1) as u8 - mv.idx;
            line &= FIELD_BOD; //5*5に収まるようにマスク
            let mut line_piece = (self.player_bods[0] | self.player_bods[1]) & line;

            //再配置を取り消す
            let piece_order: u64 = unsafe { _pext_u64(self.player_bods[0], line_piece) }; //順序記憶
            //piece_bodとplayer_bodsの中のlineに被るところを消す
            self.player_bods[0] &= !line;
            self.player_bods[1] &= !line;

            //弾く操作の逆を実行
            line_piece &= !(line & line.wrapping_neg()); //lineの最下位のbitを取得し削除
            line_piece >>= angle;
            line_piece |= target_bit; //target_bitを追加

            //再配置
            unsafe {
                self.player_bods[0] |= _pdep_u64(piece_order, line_piece);
                self.player_bods[1] |= _pdep_u64(!piece_order, line_piece);
            }
        }

        debug_assert!(
            target_bit & (self.player_bods[0] | self.player_bods[1]) == target_bit,
            "target_bit is protrude beyand piece_bod"
        );
    }
    pub fn generate_legal_move(&self, prev_move: MoveBit) -> Vec<MoveBit> {
        let mut result = Vec::new();
        let turn_player = ((-self.turn + 1) / 2) as usize;

        //setの合法手を集める
        let mut can_set_bod = !self.player_bods[turn_player];
        for angle in ANGLE {
            can_set_bod &= can_set_bod << angle;
            can_set_bod &= can_set_bod >> angle;
        }
        can_set_bod &= !self.player_bods[1 - turn_player];
        can_set_bod &= FIELD_BOD;
        for r in 0..FIELD_BOD_HEIGHT as u8 {
            for c in 0..FIELD_BOD_WIDTH as u8 {
                let idx = r * BITBOD_WIDTH as u8 + c;
                if (can_set_bod << idx) & 0b1 == 1 {
                    result.push(MoveBit::new(r, c, 8));
                }
            }
        }

        //flickの合法手を集める
        let blank: u64 = FIELD_BOD & !(self.player_bods[0] | self.player_bods[1]); //空白マス
        for angle_idx in 0..ANGLE.len() as u8 {
            let angle = ANGLE[angle_idx as usize];
            let can_flick_bod1 = self.player_bods[turn_player] & (blank << angle);
            let can_flick_bod2 = self.player_bods[turn_player] & (blank >> angle);
            for r in 0..FIELD_BOD_HEIGHT as u8 {
                for c in 0..FIELD_BOD_WIDTH as u8 {
                    let idx = r * BITBOD_WIDTH as u8 + c;
                    if (can_flick_bod1 << idx) & 0b1 == 1 {
                        result.push(MoveBit::new(r, c, angle_idx));
                    }
                    if (can_flick_bod2 << idx) & 0b1 == 1 {
                        result.push(MoveBit::new(r, c, angle_idx + 4));
                    }
                }
            }
        }

        result
    }
}
