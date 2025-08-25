use Vec;
use lru::LruCache;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::{clone, io, usize};

const ANGLES: [(isize, isize); 8] = [
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
];

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum Move {
    Place {
        r: usize,
        c: usize,
    },
    Flick {
        r: usize,
        c: usize,
        angle_idx: usize,
    },
}

impl Move {
    pub fn to_string(&self) -> String {
        match self {
            Move::Place { r, c } => format!("S({},{})", r, c),
            Move::Flick { r, c, angle_idx } => format!("F({},{},{})", r, c, angle_idx),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Snapshot {
    // turn_player: u8,
    steps: usize,
    players_has_piece: [u8; 2],
    board_data: [u8; 25],
    turn: i8,
}

#[derive(PartialEq, Eq, Clone)]
pub struct Vidro {
    // turn_player: u8,
    steps: usize,
    num_player: u8,
    players_has_piece: [u8; 2],
    board_data: [u8; 25],
    board_histroy: Vec<Snapshot>,
    prev_board: [u8; 25],
    turn: i8,
}

impl Vidro {
    pub fn new(board: u64) -> Vidro {
        let mut players_has_piece = [5; 2];
        let mut scan_board = board;
        let mut board_data = [0u8; 25];
        for idx in 0..25usize {
            scan_board >>= 2;
            let cell = scan_board & 0b11;
            board_data[idx] = cell as u8;
            match cell {
                0b01 => players_has_piece[0] -= 1,
                0b10 => players_has_piece[1] -= 1,
                _ => (),
            }
        }

        Vidro {
            turn: -(board as i8 & 0b11) * 2 + 1,
            board_data,
            steps: board as usize % 2,
            num_player: 2, //強制的2人プレイ
            players_has_piece: players_has_piece,
            board_histroy: Vec::new(),
            prev_board: [0; 25],
        }
    }
    pub fn get_hash_trout(hash: u64, v1: usize, v2: usize) -> u64 {
        let result = hash;
        result >> 2 >> 2 * (24 - (5 * v1 + v2)) & 0b11
    }
    pub fn get_hash_trout_index(hash: u64, index: usize) -> u64 {
        let result = hash;
        result >> 2 >> 2 * (24 - index)
    }
    pub fn set_hash_turn(hash: u64, turn_player: u8) -> u64 {
        let mut result = hash;
        result &= !0b11;
        result |= turn_player as u64 & 0b11;
        result
    }
    fn next_turn(&mut self) {
        self.turn = -self.turn;
    }
    fn is_there_surrounding_piece(&self, ohajiki_num: u8, coord: (usize, usize)) -> bool {
        for i in 0..3 {
            for j in 0..3 {
                if coord.0 as isize + i - 1 < 0
                    || 5 as isize <= coord.0 as isize + i - 1
                    || coord.1 as isize + j - 1 < 0
                    || 5 as isize <= coord.1 as isize + j - 1
                {
                    continue;
                }
                if self.board_data[5 * ((coord.0 as isize + i) as usize - 1)
                    + (coord.1 as isize + j) as usize
                    - 1]
                    == ohajiki_num
                {
                    return true;
                }
            }
        }
        false
    }
    fn set_ohajiki_force(&mut self, coord: (usize, usize)) {
        let now_turn_player = ((-self.turn + 1) / 2) as usize;
        let ohajiki_num = (now_turn_player + 1).try_into().unwrap();
        self.prev_board = self.board_data;

        //スナップショット保存
        self.board_histroy.push(Snapshot {
            steps: self.steps,
            players_has_piece: self.players_has_piece,
            board_data: self.board_data,
            turn: self.turn,
        });

        //変更
        self.board_data[coord.0 * 5 + coord.1] = ohajiki_num;
        self.players_has_piece[now_turn_player] -= 1;
        self.next_turn();
        self.steps += 1;
    }
    fn set_ohajiki(&mut self, coord: (usize, usize)) -> Result<(), &'static str> {
        //プレイヤーについている数字+1をそのプレイヤーの石として設計している。
        let now_turn_player = ((-self.turn + 1) / 2) as usize;
        let ohajiki_num = (now_turn_player + 1).try_into().unwrap();

        if self.board_data[coord.0 * 5 + coord.1] != 0 {
            return Err("既に石があります");
        } else if 0 < self.players_has_piece[now_turn_player] {
            if self.is_there_surrounding_piece(ohajiki_num, coord) {
                return Err("周りに既に石があります");
            } else {
                self.prev_board = self.board_data;

                //スナップショット保存
                self.board_histroy.push(Snapshot {
                    steps: self.steps,
                    players_has_piece: self.players_has_piece,
                    board_data: self.board_data,
                    turn: self.turn,
                });

                //変更
                self.board_data[coord.0 * 5 + coord.1] = ohajiki_num;
                self.players_has_piece[now_turn_player] -= 1;
                self.next_turn();
                self.steps += 1;
                return Ok(());
            }
        } else {
            return Err("もう置く石がありません");
        }
    }
    fn flick_ohajiki_force(&mut self, coord: (usize, usize), angle: (isize, isize)) {
        //スナップショット保存
        let snapshot_before_change = Snapshot {
            steps: self.steps,
            players_has_piece: self.players_has_piece,
            board_data: self.board_data,
            turn: self.turn,
        };

        let now_board = self.board_data.clone();

        let mut target = self.board_data[coord.0 * 5 + coord.1];
        let mut target_coord: (isize, isize) = (coord.0 as isize, coord.1 as isize);

        let mut next: (isize, isize); //default 処理中での移動先の座標を示す。

        while target != 0 {
            next = (target_coord.0 + angle.0, target_coord.1 + angle.1);

            if next.0 < 0 || 5 as isize <= next.0 || next.1 < 0 || 5 as isize <= next.1 {
                target = 0;
            } else {
                let u_next = (next.0 as usize, next.1 as usize);
                let u_target_coord = (target_coord.0 as usize, target_coord.1 as usize);

                match self.board_data[u_next.0 * 5 + u_next.1] {
                    0 => {
                        //移動先に何もない場合
                        self.board_data[u_next.0 * 5 + u_next.1] = target;
                        self.board_data[u_target_coord.0 * 5 + u_target_coord.1] = 0;
                        target_coord = next;
                    }
                    _ => {
                        //移動先に駒がある場合
                        target_coord = next;
                        target = self.board_data[u_next.0 * 5 + u_next.1];
                    }
                }
            }
        }

        self.next_turn();
        self.steps += 1;

        //前の手を保存
        self.prev_board = now_board;
        //スナップショット保存
        self.board_histroy.push(snapshot_before_change);
    }
    fn flick_ohajiki(
        &mut self,
        coord: (usize, usize),
        angle: (isize, isize),
    ) -> Result<(), &'static str> {
        //スナップショット保存
        let snapshot_before_change = Snapshot {
            steps: self.steps,
            players_has_piece: self.players_has_piece,
            board_data: self.board_data,
            turn: self.turn,
        };

        let now_turn_player = ((-self.turn + 1) / 2) as usize;
        let ohajiki_num: u8 = (now_turn_player + 1).try_into().unwrap();

        let now_board = self.board_data.clone();

        let mut target = self.board_data[coord.0 * 5 + coord.1];
        let mut target_coord: (isize, isize) = (coord.0 as isize, coord.1 as isize);

        if target != ohajiki_num {
            return Err("他人の駒をはじくことはできません");
        }

        let mut next: (isize, isize); //default 処理中での移動先の座標を示す。

        let mut roops = 0;

        while target != 0 {
            roops += 1;

            next = (target_coord.0 + angle.0, target_coord.1 + angle.1);

            if next.0 < 0 || 5 as isize <= next.0 || next.1 < 0 || 5 as isize <= next.1 {
                target = 0;
            } else {
                let u_next = (next.0 as usize, next.1 as usize);
                let u_target_coord = (target_coord.0 as usize, target_coord.1 as usize);

                match self.board_data[u_next.0 * 5 + u_next.1] {
                    0 => {
                        //移動先に何もない場合
                        self.board_data[u_next.0 * 5 + u_next.1] = target;
                        self.board_data[u_target_coord.0 * 5 + u_target_coord.1] = 0;
                        target_coord = next;
                    }
                    _ => {
                        //移動先に駒がある場合
                        target_coord = next;
                        target = self.board_data[u_next.0 * 5 + u_next.1];
                    }
                }
            }
        }

        if roops == 0 {
            //なにも駒がうごかないはじきは禁止
            return Err("その手はできません");
        } else {
            //駒が動かないはじきを禁止
            {
                let mut is_all = true;
                for i in 0..5 {
                    for j in 0..5 {
                        if self.board_data[i * 5 + j] != now_board[i * 5 + j] {
                            is_all = false;
                            break;
                        }
                    }
                    if !is_all {
                        break;
                    }
                }
                if is_all {
                    return Err("駒が動かないはじきはできません");
                }
            }

            //千日手の防止
            for i in 0..5 {
                for j in 0..5 {
                    if self.board_data[i * 5 + j] != self.prev_board[i * 5 + j] {
                        self.next_turn();
                        self.steps += 1;

                        //前の手を保存
                        self.prev_board = now_board;
                        //スナップショット保存
                        self.board_histroy.push(snapshot_before_change);

                        return Ok(());
                    }
                }
            }
            //千日手の制約に引っかかる場合
            for i in 0..5 {
                //元の盤面に戻す
                for j in 0..5 {
                    self.board_data[i * 5 + j] = now_board[i * 5 + j];
                }
            }
            return Err("千日手です");
        }
    }
    fn winners(&mut self) -> Eval {
        win_eval_bit_shift(self)
    }
    fn _to_string(&self) -> String {
        let vidro = self;
        let mut buf = String::new();

        buf += "now turn player: ";
        buf += &((vidro.turn - 1) / (-2)).to_string();
        buf += "\n";
        buf += "now steps: ";
        buf += &vidro.steps.to_string();

        buf += "\u{001b}[47m  0 1 2 3 4\u{001b}[0m\n";
        for i in 0..5 {
            buf += &i.to_string();

            for j in 0..5 {
                buf += "\u{001b}[";
                buf += &(30 + vidro.board_data[i * 5 + j]).to_string();
                buf += if vidro.board_data[i * 5 + j] == 0 {
                    r"m  "
                } else {
                    r"m● "
                };
                buf += COLOR_RESET;
            }
            buf += "\n";
        }

        for i in 0..vidro.num_player {
            buf += "player";
            buf += &i.to_string();
            buf += ": ";
            buf += &vidro.players_has_piece[i as usize].to_string();
            buf += "\n";
        }

        return buf;
    }
    fn to_hash(&self) -> u64 {
        let mut hash = 0u64;
        for &trout_state in &self.board_data {
            hash += trout_state as u64;
            hash <<= 2;
        }
        hash += self.steps as u64 % 2;
        hash
    }
    fn apply_move_force(&mut self, mv: &Move) {
        match mv {
            Move::Place { r, c } => self.set_ohajiki_force((*r, *c)),
            Move::Flick { r, c, angle_idx } => {
                self.flick_ohajiki_force((*r, *c), ANGLES[*angle_idx])
            }
        }
    }
    fn apply_move(&mut self, mv: &Move) -> Result<(), &'static str> {
        match mv {
            Move::Place { r, c } => self.set_ohajiki((*r, *c)),
            Move::Flick { r, c, angle_idx } => self.flick_ohajiki((*r, *c), ANGLES[*angle_idx]),
        }
    }
    fn undo_move(&mut self, _mv: &Move) -> Result<(), &'static str> {
        if let Some(s) = self.board_histroy.pop() {
            self.turn = s.turn;
            self.steps = s.steps;
            self.players_has_piece = s.players_has_piece;
            self.board_data = s.board_data;
            Ok(())
        } else {
            Err("以前の盤面データはありません。")
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

const COLOR_RESET: &str = "\u{001b}[0m";

fn _play_vidro() {
    let mut vidro = Vidro::new(0);
    let mut buf = String::new();

    let set_re = Regex::new(r"s\s+(\d+)\s+(\d+)").unwrap();
    let flick_re = Regex::new(r"f\s+(\d+)\s+(\d+)\s+(\d)").unwrap();

    let mut read_buf = String::new();

    loop {
        buf.clear();

        //盤面作成
        buf += "now turn player: ";
        buf += &(vidro.steps % (vidro.num_player as usize)).to_string();
        buf += "\n";

        buf += "\u{001b}[47m  0 1 2 3 4\u{001b}[0m\n";
        for i in 0..5 {
            buf += "\u{001b}[47m";
            buf += &i.to_string();
            buf += "\u{001b}[0m";

            for j in 0..5 {
                buf += "\u{001b}[";
                buf += &(30 + vidro.board_data[i * 5 + j]).to_string();
                buf += if vidro.board_data[i * 5 + j] == 0 {
                    r"m  "
                } else {
                    r"m● "
                };
                buf += COLOR_RESET;
            }
            buf += "\n";
        }

        for i in 0..vidro.num_player {
            buf += "player";
            buf += &i.to_string();
            buf += ": ";
            buf += &vidro.players_has_piece[i as usize].to_string();
            buf += "\n";
        }

        buf += " 3 2 1\n 4 ● 0\n 5 6 7\n";
        buf += "steps: ";
        buf += &vidro.steps.to_string();

        buf += "\nwinner: \n";

        {
            let winners = vidro.winners();
            buf += &format!("{:#?}", winners);
        }

        println!("{}", buf);

        read_buf.clear();
        loop {
            read_buf = read_buffer();

            match set_re.captures(&read_buf) {
                Some(caps) => {
                    let coord = (
                        caps[1].parse::<usize>().unwrap(),
                        caps[2].parse::<usize>().unwrap(),
                    );

                    match vidro.set_ohajiki(coord) {
                        Ok(()) => {
                            break;
                        } //成功
                        Err(err) => {
                            println!("{}", err);
                            continue;
                        }
                    }
                }
                None => (),
            }
            match flick_re.captures(&read_buf) {
                Some(caps) => {
                    let coord = (
                        caps[1].parse::<usize>().unwrap(),
                        caps[2].parse::<usize>().unwrap(),
                    );
                    let angle = caps[3].parse::<usize>().unwrap();
                    if angle < 8 {
                        match vidro.flick_ohajiki(coord, ANGLES[angle]) {
                            Ok(()) => {
                                break;
                            } //成功
                            Err(err) => {
                                println!("{}", err);
                                continue;
                            }
                        }
                    }
                }
                None => (),
            }
            println!(
                "コマンドの読み取りに失敗しました。\ncommands:\n    set y/x\n    flick y/x angle"
            );
        }
    }
}

//ここから下は探索専用
fn generate_win_masks() -> Vec<u32> {
    let mut masks = Vec::new();

    //横
    for row in 0..5 {
        for col in 0..3 {
            let mask = 0b111 << (row * 5 + col);
            masks.push(mask);
        }
    }

    //縦
    for col in 0..5 {
        for row in 0..3 {
            let mask = (1 << (row * 5 + col))
                | (1 << ((row + 1) * 5 + col))
                | (1 << ((row + 2) * 5 + col));
            masks.push(mask);
        }
    }

    // 斜め
    // 右下斜め
    for row in 0..3 {
        for col in 0..3 {
            let mask = (1 << (row * 5 + col))
                | (1 << ((row + 1) * 5 + (col + 1)))
                | (1 << ((row + 2) * 5 + (col + 2)));
            masks.push(mask);
        }
    }
    // 左下斜め
    for row in 0..3 {
        for col in 2..5 {
            let mask = (1 << (row * 5 + col))
                | (1 << ((row + 1) * 5 + (col - 1)))
                | (1 << ((row + 2) * 5 + (col - 2)));
            masks.push(mask);
        }
    }

    masks
}

use lazy_static::lazy_static;
lazy_static! {
    static ref WIN_MASKS: Vec<u32> = generate_win_masks();
}

fn static_evaluation(vidro: &mut Vidro) -> i16 {
    if let EvalValue::Win(v) = win_eval_bit_shift(&vidro).value {
        return v as i16 * 30000;
    }
    let threats = evaluate_threats(&vidro);
    let have_piece = evaluate_have_piece(&vidro);
    let position = evaluate_position(&vidro);
    let reach = evaluate_reach(vidro);
    threats + have_piece * 100 + position
}

fn evaluate_position(vidro: &Vidro) -> i16 {
    let mut score = 0;
    for i in 0..25 {
        match vidro.board_data[i] {
            1 => score += POSITION_SCORES[i], // プレイヤー1の駒
            2 => score -= POSITION_SCORES[i], // プレイヤー2の駒
            _ => (),
        }
    }

    const MARGIN_WIDTH: u64 = 9;
    let mut empty_bits = 0u64;
    let mut player_bits = [0u64; 2];
    for row in 0..5 {
        for col in 0..5 {
            let idx = row * 5 + col;
            let bit_pos = row * MARGIN_WIDTH + col; //余白を用意
            let c = vidro.board_data[idx as usize];
            if c == 0 {
                empty_bits |= 1 << bit_pos
            } else {
                player_bits[c as usize - 1] |= 1 << bit_pos;
            }
        }
    }
    const DIRS: [u64; 4] = [
        1,                // 横
        MARGIN_WIDTH,     // 縦
        MARGIN_WIDTH - 1, // 右上斜め
        MARGIN_WIDTH + 1, // 左上斜め
    ];

    //何も置かれていないマスから駒の周りのマスを消して置けるマスの数を考える
    let mut can_set_map = [empty_bits; 2];
    for p in 0..2 {
        for &dir in &DIRS {
            can_set_map[p] &= !(player_bits[p] >> dir);
            can_set_map[p] &= !(player_bits[p] << dir);
        }
    }
    score += can_set_map[0].count_ones() as i16 * vidro.players_has_piece[0] as i16;
    score -= can_set_map[1].count_ones() as i16 * vidro.players_has_piece[1] as i16;

    score
}

const POSITION_SCORES: [i16; 25] = [
    10, 0, 9, 0, 10, //
    0, 2, 4, 2, 0, //
    9, 4, 12, 4, 9, //中央 > 角 > 辺の中央 > その他
    0, 2, 4, 2, 0, //
    10, 0, 9, 0, 10, //
];

fn evaluate_have_piece(vidro: &Vidro) -> i16 {
    vidro.players_has_piece[1] as i16 - vidro.players_has_piece[0] as i16
}

fn evaluate_threats(vidro: &Vidro) -> i16 {
    const OPEN_TWO_SCORE: i16 = 0; // _XX_ (両側が空いている2)
    const SEMI_OPEN_TWO_SCORE: i16 = 300; // OXX_ や _XXO (片側が空いている2)
    const SEMI_OPEN_SPLIT_ONE_SCORE: i16 = 300; //X_X (1つ空きオープン)
    const OPEN_SPLIT_ONE_SCORE: i16 = 0; //上のX_Xに含まれる _X_X_ (1つ空きのオープンな2)

    const MARGIN_WIDTH: u64 = 9;

    let mut empty_bits = 0u64;
    let mut player_bits = [0u64; 2];

    for row in 0..5 {
        for col in 0..5 {
            let idx = row * 5 + col;
            let bit_pos = row * MARGIN_WIDTH + col; //余白bitを2つ用意
            let c = vidro.board_data[idx as usize];
            if c == 0 {
                empty_bits |= 1 << bit_pos
            } else {
                player_bits[c as usize - 1] |= 1 << bit_pos;
            }
        }
    }

    let mut total_score = 0i16;

    for p in 0..2 {
        let me = player_bits[p];
        let opp = player_bits[1 - p];
        let mut player_score = 0i16;

        // 各方向へのシフト量を定義 (7x5盤面用)
        const DIRS: [u64; 4] = [
            1,                // 横
            MARGIN_WIDTH,     // 縦
            MARGIN_WIDTH - 1, // 右上斜め
            MARGIN_WIDTH + 1, // 左上斜め
        ];

        for &d in &DIRS {
            // パターン1: オープンな2 (_XX_)
            // パターン: [空き, 自分, 自分, 空き]
            let pattern_open_two =
                (empty_bits >> 0) & (me >> d) & (me >> (d * 2)) & (empty_bits >> (d * 3));
            player_score += pattern_open_two.count_ones() as i16 * OPEN_TWO_SCORE;

            // パターン2: 片側が空いた2 (OXX_)
            // パターン: [相手, 自分, 自分, 空き]
            let pattern_semi_open_two_a =
                (opp >> 0) & (me >> d) & (me >> (d * 2)) & (empty_bits >> (d * 3));
            player_score += pattern_semi_open_two_a.count_ones() as i16 * SEMI_OPEN_TWO_SCORE;

            // パターン3: 片側が空いた2 (_XXO)
            // パターン: [空き, 自分, 自分, 相手]
            let pattern_semi_open_two_b =
                (empty_bits >> 0) & (me >> d) & (me >> (d * 2)) & (opp >> (d * 3));
            player_score += pattern_semi_open_two_b.count_ones() as i16 * SEMI_OPEN_TWO_SCORE;

            // パターン4: 1つ空きのオープンな2 (_X_X_)
            // パターン: [空き, 自分, 空き, 自分, 空き]
            let pattern_open_split_one = (empty_bits >> 0)
                & (me >> d)
                & (empty_bits >> (d * 2))
                & (me >> (d * 3))
                & (empty_bits >> (d * 4));
            player_score += pattern_open_split_one.count_ones() as i16 * OPEN_SPLIT_ONE_SCORE;

            let pattern_semi_open_split_one = me & (empty_bits >> d) & (me >> (d * 2));
            player_score +=
                pattern_semi_open_split_one.count_ones() as i16 * SEMI_OPEN_SPLIT_ONE_SCORE;
        }

        // プレイヤー1のスコアは加算、プレイヤー2のスコアは減算
        if p == 0 {
            total_score += player_score;
        } else {
            total_score -= player_score;
        }
    }

    total_score
}

fn win_eval_bit_shift(vidro: &Vidro) -> Eval {
    let mut player_bits = [0u64; 2];
    for row in 0..5 {
        for col in 0..5 {
            let idx = row * 5 + col;
            let bit_pos = row * 7 + col; //余白bitを2つ用意
            let c = vidro.board_data[idx];
            if c != 0 {
                player_bits[c as usize - 1] |= 1 << bit_pos;
            }
        }
    }

    let mut result = [false; 2];
    for p in 0..2 {
        let b = player_bits[p];

        //一列が7になっていることに注意する
        //横
        if (b & (b >> 1) & (b >> 2)) != 0 {
            result[p] = true;
        }

        //縦
        if (b & (b >> 7) & (b >> 14)) != 0 {
            result[p] = true;
        }

        //右下斜め
        if (b & (b >> 8) & (b >> 16)) != 0 {
            result[p] = true;
        }

        //左下斜め
        if (b & (b >> 6) & (b >> 12)) != 0 {
            result[p] = true;
        }
    }

    let eval: i8 = if result[0] { 1 } else { 0 } + if result[1] { -1 } else { 0 };
    let evaluated = result[0] || result[1];
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

fn win_eval(vidro: &Vidro) -> Eval {
    let mut result = [false; 2];

    let cells = &vidro.board_data;

    for i in 0..5 {
        for j in 0..5 {
            let idx = i + j * 5;
            let c = cells[idx];
            if c == 0 {
                continue;
            }

            if i < 5 - 2 {
                if c == cells[idx + 1] && c == cells[idx + 2] {
                    result[c as usize - 1] = true;
                }
            }
            if j < 5 - 2 {
                if c == cells[idx + 5] && c == cells[idx + 10] {
                    result[c as usize - 1] = true;
                }
            }
            if i < 5 - 2 && j < 5 - 2 {
                if c == cells[idx + 6] && c == cells[idx + 12] {
                    result[c as usize - 1] = true;
                }
                if c == cells[idx + 4] && c == cells[idx + 8] {
                    result[c as usize - 1] = true;
                }
            }
        }
    }

    let eval: i8 = if result[0] { 1 } else { 0 } + if result[1] { -1 } else { 0 };
    let evaluted = result[0] || result[1];
    let value = if evaluted {
        if eval == 0 {
            EvalValue::Draw
        } else {
            EvalValue::Win(eval)
        }
    } else {
        EvalValue::Unknown
    };
    Eval {
        value: value,
        evaluated: evaluted,
    }
}

const BOARD_SIZE: usize = 5;
const NUM_CELLS: usize = BOARD_SIZE * BOARD_SIZE;

const TRANSFORMS: [[usize; NUM_CELLS]; 8] = generate_transforms();

const fn generate_transforms() -> [[usize; NUM_CELLS]; 8] {
    let mut result = [[0usize; 25]; 8];
    let mut t = 0;
    while t < 8 {
        let mut base_map = [0u8; NUM_CELLS];
        let mut i = 0;
        while i < NUM_CELLS {
            base_map[i] = i as u8;
            i += 1;
        }
        let transformed = apply_transfrom(&base_map, t as u8);
        let mut j = 0;
        while j < NUM_CELLS {
            result[t][j] = transformed[j] as usize;
            j += 1;
        }
        t += 1;
    }
    result
}

fn canonical_board(board: &mut [u8; NUM_CELLS]) {
    let mut min_board = *board;
    for map in &TRANSFORMS {
        let mut transformed = [0u8; NUM_CELLS];
        for i in 0..NUM_CELLS {
            transformed[i] = board[map[i]];
        }
        if transformed < min_board {
            min_board = transformed;
        }
    }
    *board = min_board;
}

const fn apply_transfrom(board_data: &[u8; 25], t: u8) -> [u8; 25] {
    let mut result = [0u8; 25];
    let mut v1 = 0;
    while v1 < BOARD_SIZE {
        let mut v2 = 0;
        while v2 < BOARD_SIZE {
            let src_index = 5 * v1 + v2;

            let dst_index = {
                let (mut n1, mut n2) = (v1, v2);
                if t & 0b001 == 1 {
                    n1 = 4 - n1;
                }
                if t & 0b010 == 1 {
                    n2 = 4 - n2;
                }
                if t & 0b100 == 1 {
                    let tmp = n1;
                    n1 = n2;
                    n2 = tmp;
                }
                n1 * 5 + n2
            };
            result[dst_index] = board_data[src_index];
            v2 += 1;
        }
        v1 += 1;
    }
    result
}

#[derive(Clone, Debug)]
enum EvalValue {
    Win(i8),
    Draw,    //探索的千日手などの引き合分け
    Unknown, //深さ不足で未確定
}

#[derive(Clone, Debug)]
struct Eval {
    value: EvalValue,
    evaluated: bool, //評価済みかどうか
}

fn evaluate_reach(vidro: &mut Vidro) -> i16 {
    vidro.next_turn(); //意図的に手番を書き換え2手差しさせたときに勝利することがあるかを調べる
    let moves = create_legal_moves_only_flick(vidro);
    let turn = vidro.turn;
    for mv in &moves {
        vidro.apply_move_force(mv);
        if let EvalValue::Win(value) = win_eval_bit_shift(vidro).value {
            if value == turn {
                vidro.undo_move(mv).unwrap();
                vidro.next_turn();
                return value as i16
                    * (10 - vidro.players_has_piece[0] - vidro.players_has_piece[1]) as i16
                    * 15;
            }
        }
        vidro.undo_move(mv).unwrap();
    }
    vidro.next_turn();
    return 0;
}

fn find_mate_in_one_move(vidro: &mut Vidro) -> Option<Move> {
    let moves = create_legal_moves_only_flick(vidro);
    let turn = vidro.turn;
    for mv in &moves {
        vidro.apply_move_force(mv);
        if let EvalValue::Win(value) = win_eval_bit_shift(vidro).value {
            if value == turn {
                vidro.undo_move(mv).unwrap();
                return Some(*mv);
            }
        }
        vidro.undo_move(mv).unwrap();
    }
    None
}

//先後最善を指した時の詰み手順
fn find_mate_sequence(vidro: &mut Vidro, max_depth: usize) -> Option<Vec<Move>> {
    let mut sequence = Vec::new();
    //詰みがあるかどうかをしらべてある場合は手順を構築する
    let result = find_mate_sequence_recursive(vidro, max_depth, usize::MIN, usize::MAX, true);

    if let Some((_, first_move)) = result {
        //手順構築
        sequence.push(first_move);

        vidro.apply_move_force(&first_move);

        let mut idx = 0;
        while !win_eval_bit_shift(vidro).evaluated {
            if sequence.len() >= max_depth {
                break;
            }

            let remaining_depth = (max_depth - sequence.len()) as usize;

            //受け手と攻め手を入れ替え
            let is_attacker = sequence.len() % 2 == 0;
            if let Some((_, best_next_move)) = find_mate_sequence_recursive(
                vidro,
                remaining_depth,
                usize::MIN,
                usize::MAX,
                is_attacker,
            ) {
                vidro.apply_move_force(&best_next_move);
                sequence.push(best_next_move);
            } else {
                //手順が見つからなかった(バグの可能性が高い)
                for mv in sequence.iter().rev() {
                    vidro.undo_move(mv).unwrap();
                }
                return None;
            }
            idx += 1;
        }

        //見つかった手順を使って盤面を呼び出し前の状態に復元
        for mv in sequence.iter().rev() {
            vidro.undo_move(mv).unwrap();
        }

        Some(sequence)
    } else {
        None
    }
}

fn find_mate_sequence_recursive(
    vidro: &mut Vidro,
    depth: usize,
    alpha: usize,
    beta: usize,
    is_attacker: bool,
) -> Option<(usize, Move)> {
    //一手詰め判定
    if is_attacker {
        if let Some(mv) = find_mate_in_one_move(vidro) {
            return Some((depth, mv));
        }
    }

    if depth == 0 {
        return None;
    }

    let mut alpha = alpha;
    let mut beta = beta;

    if is_attacker {
        //見つかったときのdepthが大きい物(短く詰ませる)手を探す
        let attacking_moves = generate_threat_moves(vidro);
        if attacking_moves.is_empty() {
            return None;
        }

        let mut max_depth_found = usize::MIN; //最終的な詰みの深さ
        let mut best_move: Option<Move> = None;

        for mv in attacking_moves {
            vidro.apply_move_force(&mv);
            let result = find_mate_sequence_recursive(vidro, depth - 1, alpha, beta, false);
            vidro.undo_move(&mv).unwrap(); //ミスを防ぐためにすぐ戻す
            //
            // 相手の手番で再帰呼び出し
            if let Some((found_depth, _)) = result {
                if max_depth_found < found_depth {
                    //最善が更新された
                    max_depth_found = found_depth;
                    best_move = Some(mv.clone());
                }
                alpha = alpha.max(max_depth_found);
                if alpha >= beta {
                    break;
                }
            }
        }

        best_move.map(|mv| (max_depth_found, mv))
    } else {
        //見つかったときのdepthが小さい物(長く詰まされる)手を探す
        //特に効率の良い守る手を見つける方法はないため合法手から絞り込むことにする
        let defending_moves = create_legal_moves(vidro);
        //合法手が一つもないということは起きないため空の場合は考えない
        // if defending_moves.is_empty() {
        // return Some(depth + 1);
        // }

        let mut min_depth_found = usize::MAX;
        let mut best_move: Option<Move> = None;

        //相手の手番で再帰呼び出し
        for mv in defending_moves {
            vidro.apply_move_force(&mv);
            let result = find_mate_sequence_recursive(vidro, depth - 1, alpha, beta, true);
            vidro.undo_move(&mv).unwrap();

            if result.is_none() {
                return None;
            }

            if let Some((found_depth, _)) = result {
                //詰む場合
                if found_depth < min_depth_found {
                    //最善が更新された
                    min_depth_found = found_depth;
                    best_move = Some(mv.clone());
                }
                beta = beta.min(min_depth_found);
                if beta <= alpha {
                    break;
                }
            }
        }

        //ここで詰まない場合が一つもない
        //すなわち必ず詰むのでvalueを返却
        best_move.map(|mv| (min_depth_found, mv))
    }
}

// main関数などから呼び出すためのラッパー関数
fn find_mate(vidro: &mut Vidro, max_depth: usize) -> Option<Move> {
    let mut mate_move = Move::Place { r: 0, c: 0 };
    if find_mate_recursive(vidro, max_depth, &mut mate_move) {
        Some(mate_move)
    } else {
        None
    }
}

// 詰み探索の本体（再帰関数）
fn find_mate_recursive(vidro: &mut Vidro, depth: usize, mate_move: &mut Move) -> bool {
    //深さ切れ(詰みなしと判断)
    if depth == 0 {
        return false;
    }

    let attacking_moves = generate_threat_moves(vidro);
    if attacking_moves.is_empty() {
        return false; //詰めろを掛けられない
    }

    //OR探索
    for mv in attacking_moves {
        vidro.apply_move_force(&mv);

        //受けが無くなっているかどうかを調べる
        if check_opponent_defense(vidro, depth - 1, mate_move) {
            //受けがないことが確定 == 詰みが見つかった
            vidro.undo_move(&mv).unwrap();
            *mate_move = mv.clone(); //最後の代入の値==最初に指す手==詰み手順に入る時の手
            return true;
        }
        vidro.undo_move(&mv).unwrap();
    }

    //どの手も詰みにならなかった
    false
}

//NOTE! 詰みの読み筋を相手の物も含めるようにする

//受けがないかどうか
fn check_opponent_defense(vidro: &mut Vidro, depth: usize, mate_move: &mut Move) -> bool {
    //勝になっていないかを確認
    if let EvalValue::Win(v) = win_eval_bit_shift(vidro).value {
        if v == -vidro.turn {
            return true;
        }
    }

    if depth == 0 {
        return false;
    }

    let defending_moves = generate_defense_moves(vidro);
    if defending_moves.is_empty() {
        //受けなし
        return true;
    }

    // 生成した受け手の全ての応手に対して、詰み手順が続くか調べる (AND検索)
    for mv in defending_moves {
        vidro.apply_move_force(&mv);

        // 自分が再度攻めて詰むかどうかを再帰的に調べる
        let can_mate = find_mate_recursive(vidro, depth - 1, mate_move);

        vidro.undo_move(&mv).unwrap();

        if !can_mate {
            // 相手のこの受けで詰みが途切れた。
            // したがって、元の自分の手は必勝の詰み手順ではない。
            return false;
        }
    }

    // 相手がどう受けても、全て詰み手順が続くことが証明された
    true
}

fn is_reach(vidro: &mut Vidro) -> bool {
    vidro.next_turn(); //意図的に手番を書き換え2手差しさせたときに勝利することがあるかを調べる
    let result = checkmate_in_one_move(vidro);
    vidro.next_turn(); //手番を戻す
    result
}

fn checkmate_in_one_move(vidro: &mut Vidro) -> bool {
    let moves = create_legal_moves_only_flick(vidro);
    let turn = vidro.turn;
    for mv in &moves {
        vidro.apply_move_force(mv);
        if let EvalValue::Win(value) = win_eval_bit_shift(vidro).value {
            if value == turn {
                vidro.undo_move(mv).unwrap();
                return true;
            }
        }
        vidro.undo_move(mv).unwrap();
    }
    false
}

fn generate_threat_moves(vidro: &mut Vidro) -> Vec<Move> {
    let mut moves = create_legal_moves(vidro);
    moves.retain(|mv| {
        vidro.apply_move_force(mv);
        //詰めろ(自殺手を除く)
        if is_reach(vidro) && !checkmate_in_one_move(vidro) {
            vidro.undo_move(mv).unwrap();
            return true;
        }
        vidro.undo_move(mv).unwrap();
        false
    });
    moves
}

fn generate_defense_moves(vidro: &mut Vidro) -> Vec<Move> {
    let mut moves = create_legal_moves(vidro);
    moves.retain(|mv| {
        vidro.apply_move_force(mv);
        if !checkmate_in_one_move(vidro) {
            vidro.undo_move(mv).unwrap();
            return true;
        }
        vidro.undo_move(mv).unwrap();
        false
    });
    moves
}

fn quick_eval(board: &Vidro) -> i8 {
    let mut eval1 = 0i8;
    for i in 0..9 {
        eval1 += board.board_data[(i % 3) * 2 + (i / 3) * 10] as i8 * (-2) + 3; //角と辺の中央の評価をあげる
    }
    (board.players_has_piece[1] as i8 - board.players_has_piece[0] as i8) + eval1
}

fn order_children(children: &mut Vec<Vidro>, turn: u8) {
    children.sort_by_key(|board| {
        let val = quick_eval(board);
        if turn == 0 { -val } else { val }
    });
}

fn create_legal_moves_only_flick(target_board: &mut Vidro) -> Vec<Move> {
    let mut movable: Vec<Move> = Vec::new();

    //可能な限りの子を作成
    for i in 0..5 {
        for j in 0..5 {
            for a in 0..8 {
                if let Ok(()) = target_board.flick_ohajiki((i, j), ANGLES[a]) {
                    //テキトー置きが成功したとき
                    let mv = Move::Flick {
                        r: i,
                        c: j,
                        angle_idx: a,
                    };
                    target_board.undo_move(&mv).unwrap(); //変更が加わってしまった盤面を元に戻す
                    movable.push(mv);
                }
            }
        }
    }
    return movable;
}

fn create_legal_moves(target_board: &mut Vidro) -> Vec<Move> {
    let mut movable: Vec<Move> = Vec::new();

    //可能な限りの子を作成
    for i in 0..5 {
        for j in 0..5 {
            if let Ok(()) = target_board.set_ohajiki((i, j)) {
                //テキトー置きが成功したとき
                let mv = Move::Place { r: i, c: j };
                target_board.undo_move(&mv).unwrap(); //変更が加わってしまった盤面を元に戻す
                movable.push(mv);
            }
        }
    }
    for i in 0..5 {
        for j in 0..5 {
            for a in 0..8 {
                if let Ok(()) = target_board.flick_ohajiki((i, j), ANGLES[a]) {
                    //テキトー置きが成功したとき
                    let mv = Move::Flick {
                        r: i,
                        c: j,
                        angle_idx: a,
                    };
                    target_board.undo_move(&mv).unwrap(); //変更が加わってしまった盤面を元に戻す
                    movable.push(mv);
                }
            }
        }
    }
    return movable;
}

fn evaluate_for_negamax(board: &mut Vidro) -> i16 {
    static_evaluation(board) * board.turn as i16
}

#[derive(Clone, Default)]
pub struct SearchInfo {
    pub depth: usize,
    pub score: i16,
    pub pv: Vec<Move>,
    pub nodes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TTFlag {
    Exact,      // このスコアは真の評価値 (alpha < score < beta)
    LowerBound, // このスコアは下限値 (score >= beta, betaカットで得られた)
    UpperBound, // このスコアは上限値 (score <= alpha, 有望な手が見つからなかった)
}

// 置換表に保存するデータ構造
#[derive(Clone, Copy, Debug)]
struct TTEntry {
    score: i16,
    depth: u8, // 保存したときの探索深さ
    flag: TTFlag,
    best_move: Move, // その局面で見つかった最善手
}

const USE_CACHE: bool = true;
const USE_CACHE_DEPTH: usize = 8;

const DRAW_SCORE: i16 = 0;
const WIN_LOSE_SCORE: i16 = 30000;

fn alphabeta(
    board: &mut Vidro,
    depth: usize,
    mut alpha: i16,
    mut beta: i16,
    tt: &mut LruCache<u64, TTEntry>,
    route: &mut Vec<u64>,
    process: &mut Progress,
    is_root: bool, // ★自分がルートノード（探索の起点）かを知るためのフラグ
    shared_info: Arc<Mutex<SearchInfo>>, // ★情報共有のための構造体
) -> (i16, Vec<Move>) {
    process.update(depth, board, tt.len());
    let mut best_pv = Vec::new();
    let mut canonical_board_data = board.board_data;
    canonical_board(&mut canonical_board_data);
    let hash = board.to_hash();
    //千日手判定
    if route.contains(&hash) {
        return (DRAW_SCORE, Vec::new()); //引き分け評価
    }
    route.push(hash);

    //自己評価
    let terminal_eval = win_eval_bit_shift(board);
    if terminal_eval.evaluated {
        route.pop();
        let score = if let EvalValue::Win(v) = terminal_eval.value {
            v as i16 * WIN_LOSE_SCORE
        } else {
            DRAW_SCORE
        };
        return (score, Vec::new());
    }

    if depth == 0 {
        route.pop();
        let static_score = evaluate_for_negamax(board);

        if static_score * board.turn as i16 > 1000 {
            if let Some(mate_sequence) = find_mate_sequence(board, 9) {
                // 詰みを発見！スコアを「勝ち」に格上げし、手順も返す
                let mate_score = WIN_LOSE_SCORE - mate_sequence.len() as i16;
                return (mate_score, mate_sequence);
            }
        }
        return (static_score, best_pv);
    }

    let original_alpha = alpha;
    let mut best_move_from_tt: Option<Move> = None;
    //置換表参照
    if USE_CACHE {
        if let Some(entry) = tt.get(&hash) {
            if entry.depth as usize >= depth {
                match entry.flag {
                    TTFlag::Exact => return (entry.score, vec![entry.best_move]),
                    TTFlag::LowerBound => alpha = alpha.max(entry.score),
                    TTFlag::UpperBound => beta = beta.min(entry.score),
                }
                if alpha >= beta {
                    return (entry.score, vec![entry.best_move]);
                }
            }
            best_move_from_tt = Some(entry.best_move);
        }
    }

    let mut moves = create_legal_moves(board);
    if let Some(tt_move) = best_move_from_tt {
        if let Some(pos) = moves.iter().position(|m| *m == tt_move) {
            let m = moves.remove(pos);
            moves.insert(0, m);
        }
    }

    let mut best_score = i16::MIN;
    let mut best_move: Option<Move> = None;

    for mv in &moves {
        //手を実行
        board.apply_move_force(mv);
        //その手ができた場合
        let (mut score, mut child_pv) = alphabeta(
            board,
            depth - 1,
            -beta,
            -alpha,
            tt,
            route,
            process,
            false,
            shared_info.clone(),
        );
        score = -score;
        board.undo_move(&mv).unwrap(); //元に戻す
        if best_score < score {
            best_score = score;
            best_move = Some(*mv);
            best_pv.clear();
            best_pv.push(*mv);
            best_pv.append(&mut child_pv); // 本格的には子ノードのPVも連結する

            if is_root {
                let mut info = shared_info.lock().unwrap();
                info.score = best_score;
                info.pv = best_pv.clone();
                info.nodes = process.nodes_searched;
            }
        }
        alpha = alpha.max(best_score);
        if alpha >= beta {
            break; //beta cut
        }
    }

    if USE_CACHE {
        if let Some(mv) = best_move {
            let flag = if best_score <= original_alpha {
                TTFlag::UpperBound
            } else if best_score >= beta {
                TTFlag::LowerBound
            } else {
                TTFlag::Exact
            };
            let new_entry = TTEntry {
                best_move: mv,
                score: best_score,
                depth: depth as u8,
                flag,
            };
            tt.put(hash, new_entry);
        }
    }

    route.pop(); // 探索パスから除去して戻る
    (best_score, best_pv)
}

struct Progress {
    nodes_searched: usize,
    last_print: Instant,
}

impl Progress {
    fn new() -> Self {
        Self {
            nodes_searched: 0,
            last_print: Instant::now(),
        }
    }

    fn update(&mut self, current_depth: usize, board: &Vidro, tt_len: usize) {
        self.nodes_searched += 1;
        let now = Instant::now();
        if now.duration_since(self.last_print) >= Duration::from_secs(10) {
            // println!(
            //     "探索ノード数: {}, 現在深さ: {}, TT size:{}",
            //     self.nodes_searched, current_depth, tt_len
            // );
            // println!("{}", board._to_string());
            // self.last_print = now;
        }
    }
}

fn main() {
    // _play_vidro();
    // return;
    //
    //テストの局面(詰み)
    // vidro.set_ohajiki((0, 2)).unwrap();
    // vidro.set_ohajiki((2, 0)).unwrap();
    // vidro.set_ohajiki((2, 4)).unwrap();
    // vidro.set_ohajiki((0, 0)).unwrap();
    // vidro.set_ohajiki((0, 4)).unwrap();
    // vidro.set_ohajiki((4, 0)).unwrap();

    //問題の局面
    // vidro.set_ohajiki((2, 2)).unwrap();
    // vidro.set_ohajiki((0, 0)).unwrap();
    // vidro.set_ohajiki((0, 4)).unwrap();
    // vidro.set_ohajiki((2, 0)).unwrap();
    // vidro.set_ohajiki((2, 4)).unwrap();
    // vidro.set_ohajiki((1, 2)).unwrap();
    // vidro.set_ohajiki((1, 0)).unwrap();
    // vidro.set_ohajiki((4, 0)).unwrap();
    // vidro.set_ohajiki((3, 0)).unwrap();
    // vidro.set_ohajiki((4, 2)).unwrap();

    // println!("{}", vidro._to_string());

    // vidro.set_ohajiki((0, 0)).unwrap();
    // vidro.set_ohajiki((4, 4)).unwrap();
    // vidro.set_ohajiki((0, 2)).unwrap();
    // vidro.set_ohajiki((4, 2)).unwrap();
    // vidro.set_ohajiki((2, 1)).unwrap();
    // vidro.set_ohajiki((2, 3)).unwrap();

    // println!("{:#?}", find_mate(&mut vidro, 9));
    // println!("{:#?}", find_mate_sequence(&mut vidro, 9));
    let capacity = NonZeroUsize::new(100000).unwrap();

    let mut vidro = Vidro::new(0);

    let shared_info = Arc::new(Mutex::new(SearchInfo::default()));
    let info_clone_for_ui = shared_info.clone();

    let mut vidro_for_search = vidro.clone();
    let mut route: Vec<u64> = Vec::new();
    let depth = 50;

    let mut best_move: Option<Move> = None;

    let search_thread = thread::spawn(move || {
        let mut tt: LruCache<u64, TTEntry> = LruCache::new(NonZeroUsize::new(100_000).unwrap());
        let mut process = Progress::new();

        let mut best_move_overall: Option<Move> = None;

        //反復深化ループ
        for depth_run in 1..=50 {
            let mut route = Vec::new();

            //ルートノードで探索
            let (score, pv_sequence) = alphabeta(
                &mut vidro_for_search,
                depth_run,
                i16::MIN + 1,
                i16::MAX,
                &mut tt,
                &mut route,
                &mut process,
                true,
                shared_info.clone(),
            );

            //結果をUIに通知
            {
                let mut info = shared_info.lock().unwrap();
                info.score = score;
                info.depth = depth_run;
                info.pv = pv_sequence.clone();
                info.nodes = process.nodes_searched;
            }

            if !pv_sequence.is_empty() {
                best_move_overall = Some(pv_sequence[0].clone());
            }

            if score.abs() >= 29000 {
                //詰み発見
                break;
            }
        }

        //最終的な最善手を返す
        best_move_overall
    });

    println!("探索開始...");
    loop {
        thread::sleep(Duration::from_millis(200));
        {
            let info = info_clone_for_ui.lock().unwrap();
            print!(
                "\rDepth: {:2}, Score: {:6}, Nodes: {:8}, PV: {:<50}",
                info.depth,
                info.score,
                info.nodes,
                info.pv
                    .iter()
                    .map(|m| m.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            );

            // 標準出力をフラッシュして即時表示
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        }

        if search_thread.is_finished() {
            break;
        }
    }

    //探索スレッドの終了を待って最善手を取得
    let final_best_move = search_thread.join().unwrap();
    println!("\n探索終了");
    println!("最終的な最善手: {:?}", final_best_move);
}
