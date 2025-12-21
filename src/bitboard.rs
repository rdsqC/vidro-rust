use arrayvec::ArrayVec;

use crate::{
    eval_value::{Eval, EvalValue},
    snapshot::BoardSnapshot,
    snapshot_features::BitIter,
};

#[derive(Debug, Clone, Copy)]
pub struct Bitboard {
    pub player_bods: [u64; 2],
    pub have_piece: [u8; 2],
    pub turn: i8, // 1が先手, -1が後手
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MoveBit {
    pub idx: u8,
    pub angle_idx: u8, //8以上のときはset
}

impl MoveBit {
    pub fn to_string(&self) -> String {
        let r = self.idx / BITBOD_WIDTH as u8;
        let c = self.idx % BITBOD_WIDTH as u8;
        if self.angle_idx < 8 {
            //flick
            format!("F({},{},{})", r, c, self.angle_idx)
        } else {
            format!("S({},{})", r, c)
        }
    }
    pub fn vec_to_string(moves: &Vec<MoveBit>) -> String {
        let mut result = String::new();
        for &mv in moves {
            result += &mv.to_string();
            result += ", ";
        }
        result
    }
    pub fn print_vec_to_string(moves: &Vec<MoveBit>) {
        let text = Self::vec_to_string(moves);
        println!("legal_moves: {}\nlen: {}", text, moves.len());
    }
    pub fn new(r: u8, c: u8, idx: u8) -> MoveBit {
        Self {
            idx: r * BITBOD_WIDTH as u8 + c,
            angle_idx: idx,
        }
    }
    pub fn from_idx(idx: u8, angle_idx: u8) -> Self {
        Self { idx, angle_idx }
    }
    pub fn field_idx(&self) -> usize {
        self.idx as usize / BITBOD_WIDTH as usize * FIELD_BOD_WIDTH as usize
            + self.idx as usize % BITBOD_WIDTH as usize
    }
}

pub type MoveList = ArrayVec<MoveBit, 128>;

pub const BITBOD_WIDTH: u64 = 9;
pub const FIELD_BOD_WIDTH: u64 = 5;
pub const FIELD_BOD_HEIGHT: u64 = 5;
pub const FIELD_BOD: u64 = {
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

    #[inline(always)]
    pub fn get_turn_idx(&self) -> usize {
        ((-self.turn + 1) / 2) as usize
    }

    //千日手の場合はtrue, そうでない手をfalseと返す
    pub fn check_illegal_move(&mut self, mv: MoveBit, prev_hash: Option<u64>) -> bool {
        self.apply_force(mv);
        let is_illegal = prev_hash.is_some_and(|prev| self.to_compression_bod() == prev);
        self.undo_force(mv);
        is_illegal
    }
    pub fn apply_force_with_check_illegal_move(
        &mut self,
        mv: MoveBit,
        prev_hash: Option<u64>,
    ) -> Result<(), ()> {
        self.apply_force(mv);
        if prev_hash.is_some_and(|prev| self.to_compression_bod() == prev) {
            self.undo_force(mv);
            Err(())
        } else {
            Ok(())
        }
    }
    pub fn apply_and(&mut self, mv: MoveBit, prev_hash: Option<u64>, f: impl Fn(&mut Self) -> ()) {
        self.apply_force(mv);
        if prev_hash.is_some_and(|prev| self.to_compression_bod() == prev) {
            self.undo_force(mv);
        } else {
            f(self);
        }
    }
    fn apply_force(&mut self, mv: MoveBit) {
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
        let turn_player = self.get_turn_idx();
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
    pub fn game_over(&self) -> bool {
        for p in 0..2 {
            let b = self.player_bods[p];

            //一列が7になっていることに注意する
            //横
            if (b & (b >> ANGLE[0]) & (b >> ANGLE[0] * 2)) != 0 {
                return true;
            }

            //縦
            if (b & (b >> ANGLE[2]) & (b >> ANGLE[2] * 2)) != 0 {
                return true;
            }

            //右下斜め
            if (b & (b >> ANGLE[1]) & (b >> ANGLE[1] * 2)) != 0 {
                return true;
            }

            //左下斜め
            if (b & (b >> ANGLE[3]) & (b >> ANGLE[3] * 2)) != 0 {
                return true;
            }
        }
        false
    }
    pub fn bod_legal_set_moves_with_turn_idx(&self, turn_idx: usize) -> u64 {
        if 0 == self.have_piece[turn_idx] {
            return 0u64;
        }

        let mut set_bod = self.player_bods[turn_idx];

        //上下スライド, そのあとの左右スライドで3*3の範囲のbodに加工
        set_bod |= set_bod << 1 | set_bod >> 1;
        set_bod |= set_bod << BITBOD_WIDTH | set_bod >> BITBOD_WIDTH;

        //相手と自分の駒とその周囲8マスを覗いたfieldのマス
        set_bod = !(set_bod | self.player_bods[1 - turn_idx]) & FIELD_BOD;

        set_bod
    }
    //千日手除外の処理は探索に任せる
    pub fn bod_legal_flick_moves_with_turn_idx(&self, turn_idx: usize) -> [u64; 8] {
        let mut flick_bod: [u64; 8] = [0; 8];
        let all_piece = self.player_bods[0] | self.player_bods[1];
        let blank = FIELD_BOD & !all_piece;
        let p_bod = self.player_bods[turn_idx];

        for i in 0..ANGLE.iter().len() {
            let mut bod1 = 0;
            let mut bod2 = 0;
            for step in 1..4 {
                bod1 |= blank >> ANGLE[i] * step;
                bod2 |= blank << ANGLE[i] * step;
            }

            bod1 &= p_bod;
            bod2 &= p_bod;

            flick_bod[i] = bod1;
            flick_bod[i + 4] = bod2;
        }

        flick_bod
    }
    pub fn bod_legal_set_moves(&self) -> u64 {
        self.bod_legal_set_moves_with_turn_idx(self.get_turn_idx())
    }
    pub fn bod_legal_flick_moves(&self) -> [u64; 8] {
        self.bod_legal_flick_moves_with_turn_idx(self.get_turn_idx())
    }
    pub fn bod_blankplace_win(&self, turn_idx: usize) -> u64 {
        let p_bod = self.player_bods[turn_idx];
        let mut result = 0u64;
        for angle in ANGLE {
            result |= p_bod << angle & p_bod >> angle;
        }
        result &= !(self.player_bods[0] | self.player_bods[1]);
        result
    }
    pub fn can_set_count_with_turn_idx(&self, turn_player: usize) -> u32 {
        //setの合法手を集める
        self.bod_legal_set_moves_with_turn_idx(turn_player)
            .count_ones()
    }
    pub fn win_turn(&self) -> i16 {
        let mut result = [0i16; 2];
        for p in 0..2 {
            let b = self.player_bods[p];

            //一列が7になっていることに注意する
            //横
            if (b & (b >> ANGLE[0]) & (b >> ANGLE[0] * 2)) != 0 {
                result[p] = 1;
            }

            //縦
            if (b & (b >> ANGLE[2]) & (b >> ANGLE[2] * 2)) != 0 {
                result[p] = 1;
            }

            //右下斜め
            if (b & (b >> ANGLE[1]) & (b >> ANGLE[1] * 2)) != 0 {
                result[p] = 1;
            }

            //左下斜め
            if (b & (b >> ANGLE[3]) & (b >> ANGLE[3] * 2)) != 0 {
                result[p] = 1;
            }
        }

        result[0] - result[1]
    }
    pub fn win_eval(&self) -> Eval {
        let mut result = [0i16; 2];
        for p in 0..2 {
            let b = self.player_bods[p];

            //一列が7になっていることに注意する
            //横
            if (b & (b >> ANGLE[0]) & (b >> ANGLE[0] * 2)) != 0 {
                result[p] = 1;
            }

            //縦
            if (b & (b >> ANGLE[2]) & (b >> ANGLE[2] * 2)) != 0 {
                result[p] = 1;
            }

            //右下斜め
            if (b & (b >> ANGLE[1]) & (b >> ANGLE[1] * 2)) != 0 {
                result[p] = 1;
            }

            //左下斜め
            if (b & (b >> ANGLE[3]) & (b >> ANGLE[3] * 2)) != 0 {
                result[p] = 1;
            }
        }

        let eval = result[0] - result[1];
        let evaluated = result[0] + result[1] > 0;
        let value = if evaluated {
            if eval == 0 {
                EvalValue::Draw
            } else {
                EvalValue::Win(eval)
            }
        } else {
            EvalValue::Unknown
        };
        Eval { value, evaluated }
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
    pub fn iter_legal_move(&self) -> impl Iterator<Item = MoveBit> + use<> {
        let turn_idx = self.get_turn_idx();

        self.iter_legal_set_move_with_turn_idx(turn_idx)
            .chain(self.iter_legal_flick_move_with_turn_idx(turn_idx))
    }
    pub fn iter_legal_set_move_with_turn_idx(
        &self,
        turn_idx: usize,
    ) -> impl Iterator<Item = MoveBit> + use<> {
        BitIter::new(self.bod_legal_set_moves_with_turn_idx(turn_idx)).map(move |idx| MoveBit {
            idx: idx as u8,
            angle_idx: 8,
        })
    }
    pub fn iter_legal_flick_move_with_turn_idx(
        &self,
        turn_idx: usize,
    ) -> impl Iterator<Item = MoveBit> + use<> {
        let bods = self.bod_legal_flick_moves_with_turn_idx(turn_idx);
        bods.into_iter()
            .enumerate()
            .flat_map(move |(angle_idx, bod)| {
                BitIter(bod).map(move |idx| MoveBit {
                    idx: idx as u8,
                    angle_idx: angle_idx as u8,
                })
            })
    }
    pub fn iter_legal_set_move(&self) -> impl Iterator<Item = MoveBit> + use<> {
        self.iter_legal_set_move_with_turn_idx(self.get_turn_idx())
    }
    pub fn iter_legal_flick_move(&self) -> impl Iterator<Item = MoveBit> + use<> {
        self.iter_legal_flick_move_with_turn_idx(self.get_turn_idx())
    }
    pub fn generate_legal_moves(&self, out: &mut MoveList) {
        out.extend(self.iter_legal_move());
    }
    pub fn to_compression_bod(&self) -> u64 {
        use std::arch::x86_64::_pext_u64;
        let mut result = 0u64;
        let turn_player = ((-self.turn + 1) / 2) as usize;
        unsafe {
            result |= _pext_u64(self.player_bods[0], FIELD_BOD)
                << (FIELD_BOD_WIDTH * FIELD_BOD_HEIGHT + 1);
            result |= _pext_u64(self.player_bods[1], FIELD_BOD) << 1;
            result |= turn_player as u64;
        }
        // println!("{:0>64b}", result);
        result
    }
    pub fn to_snapshot(&self, prev_hash: Option<u64>) -> BoardSnapshot {
        BoardSnapshot {
            p1: self.player_bods[0],
            p2: self.player_bods[1],
            turn: self.turn,
            p1_hand_piece: self.have_piece[0],
            p2_hand_piece: self.have_piece[1],
            prev_hash,
        }
    }
}
