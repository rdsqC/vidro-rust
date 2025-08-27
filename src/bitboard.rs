use regex::Regex;
use std::io;

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
            line >>= BITBOD_WIDTH * (4 - c) + (4 - r);
            line &= FIELD_BOD; //5*に収まるようにマスク
            let mut line_piece = self.piece_bod & line;

            //各駒のうちの駒種類の振り分けを記憶
            let piece_order: u64 = unsafe { _pext_u64(self.player_bods[0], line_piece) };

            //piece_bodとplayer_bodsの中のlineに被るところを消す
            self.piece_bod &= !line;
            self.player_bods[0] &= !line;
            self.player_bods[1] &= !line;

            //弾く操作を実行
            line_piece ^= target_bit; //target_bitを消す
            line_piece >>= angle;
            line_piece |= line & line.wrapping_neg(); //lineの最下位のbitを取得し追加

            //再配置
            self.piece_bod |= line_piece;
            unsafe {
                self.player_bods[0] |= _pdep_u64(piece_order, self.piece_bod);
                self.player_bods[1] |= _pdep_u64(piece_order, self.piece_bod);
            }
        }
    }
}

fn read_buffer() -> String {
    let mut buffer = String::new();
    io::stdin()
        .read_line(&mut buffer)
        .expect("Failed to read line.");
    buffer.trim().to_string()
}

pub trait BitboardConsole {
    fn to_string(&self) -> String;
    fn read_to_move() -> Move;
}

impl BitboardConsole for Bitboard {
    fn to_string(&self) -> String {
        const COLOR_RESET: &str = "\u{001b}[0m";
        let mut buf = String::new();

        buf += "\n--------------------";
        buf += "\nnow turn player: ";
        buf += &self.turn_player.to_string();
        buf += "\n";

        buf += "  0 1 2 3 4\n";
        for c in 0..5 {
            buf += &c.to_string();

            for r in 0..5 {
                buf += "\u{001b}[";
                buf += &(31 + ((self.player_bods[0] >> (c * BITBOD_WIDTH + r)) & 0b1)).to_string();
                buf += if (self.piece_bod >> (c * BITBOD_WIDTH + r)) & 0b1 == 0 {
                    r"m  "
                } else {
                    r"m● "
                };
                buf += COLOR_RESET;
            }
            buf += "\n";
        }

        for i in 0..2 {
            buf += "player";
            buf += &i.to_string();
            buf += ": ";
            buf += &self.have_piece[i as usize].to_string();
            buf += "\n";
        }

        return buf;
    }
    fn read_to_move() -> Move {
        let set_re = Regex::new(r"S\s+(\d+)\s+(\d+)").unwrap();
        let flick_re = Regex::new(r"F\s+(\d+)\s+(\d+)\s+(\d)").unwrap();

        loop {
            let read_buf = read_buffer();

            match set_re.captures(&read_buf) {
                Some(caps) => {
                    return Move::Place {
                        r: caps[1].parse::<u64>().unwrap(),
                        c: caps[2].parse::<u64>().unwrap(),
                    };
                }
                None => (),
            }
            match flick_re.captures(&read_buf) {
                Some(caps) => {
                    return Move::Flick {
                        r: caps[1].parse::<u64>().unwrap(),
                        c: caps[2].parse::<u64>().unwrap(),
                        angle_idx: caps[3].parse::<usize>().unwrap(),
                    };
                }
                None => (),
            }
            println!(
                "コマンドの読み取りに失敗しました。\ncommands:\n    set y/x\n    flick y/x angle"
            );
        }
    }
}
