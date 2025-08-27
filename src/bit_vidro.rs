pub struct BitVidro {
    player_bods: [u64; 2],
    piece_bod: u64,
    have_piece: [i64; 2],
    turn: i64,
    turn_player: usize,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum Move {
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

const BITBOD_WIDTH: u64 = 10;
const FIELD_BOD_WIDTH: u64 = 5;
const FIELD_BOD_HEIGHT: u64 = 5;
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
    while count < 8 {
        let mut steps = 0u64; //target_bit は含まない
        while steps < 5 {
            result[count] |= (1u64 << 24) >> (ANGLE[count - 4] * steps);
            steps += 1;
        }
        count += 1;
    }
    result
};

impl BitVidro {
    pub fn new(player_bods: [u64; 2], turn: i64) -> Self {
        BitVidro {
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
        BitVidro {
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
    pub fn apply_force(mv: &Move) {}
    pub fn set_force(&mut self, c: u64, r: u64) {
        self.player_bods[self.turn_player] |= 1u64 << (c * BITBOD_WIDTH + r);
        self.piece_bod |= 1u64 << (c * BITBOD_WIDTH + r);
        self.have_piece[self.turn_player] -= 1;
        self.turn_change();
    }
    pub fn flick_force(&mut self, c: u64, r: u64, angle_idx: usize) {
        use std::arch::x86_64::{_pdep_u64, _pext_u64};

        let angle = ANGLE[angle_idx % 4];
        let is_positive_angle = angle_idx < 4;
        let mut line = ANGLE_LINE[angle_idx];
        let target_bit = 1u64 << (BITBOD_WIDTH * c + r);

        if is_positive_angle {
            //左シフトで表す方向
            //駒の場所にlineの先端を移動する
            line <<= BITBOD_WIDTH * c + r;
            line &= FIELD_BOD; //5*5に収まるようにマスク
            let mut line_piece = self.piece_bod & line;

            //各駒のうちの駒種類の振り分けを記憶
            let piece_order: u64 = unsafe { _pext_u64(self.player_bods[0], line_piece) };

            //piece_bodとplayer_bodsの中のlineに被るところを消す
            self.piece_bod &= !line;
            self.player_bods[0] &= !line;
            self.player_bods[1] &= !line;

            //弾く操作をシミュレーション
            line_piece ^= target_bit; //target_bitを消す。
            line_piece >>= angle;
            line_piece |= line & line.wrapping_neg(); //lineの最下位のbitを取得し追加

            //再配置
            self.piece_bod |= line_piece;
            unsafe {
                self.player_bods[0] |= _pdep_u64(piece_order, line_piece);
                self.player_bods[1] |= _pdep_u64(!piece_order, line_piece);
            }
        } else {
            //右シフトで表す方向
            line >>= (BITBOD_WIDTH * (4 - c) + (4 - r));
        }
    }
}
