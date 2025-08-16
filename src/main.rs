use Vec;
use lru::LruCache;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::io;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

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

#[derive(Clone, PartialEq, Hash, Eq)]
pub struct Vidro {
    turn: u8,
    steps: usize,
    prev_board: [u8; 25],
    num_player: u8,
    players_has_piece: [u8; 2],
    board_data: [u8; 25],
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
            turn: board as u8 & 0b11,
            board_data,
            steps: board as usize % 2,
            prev_board: [0; 25],
            num_player: 2, //強制的2人プレイ
            players_has_piece: players_has_piece,
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
    pub fn set_hash_turn(hash: u64, turn: u8) -> u64 {
        let mut result = hash;
        result &= !0b11;
        result |= turn as u64 & 0b11;
        result
    }
    fn set_turn(&mut self, turn: u8) {
        self.turn = turn;
    }
    fn next_turn(&mut self) {
        self.turn = 1 - self.turn;
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
    fn set_ohajiki(&mut self, coord: (usize, usize)) -> Result<(), &'static str> {
        //プレイヤーについている数字+1をそのプレイヤーの石として設計している。
        let now_turn_player = self.turn as usize;
        let ohajiki_num = (now_turn_player + 1).try_into().unwrap();

        if self.board_data[coord.0 * 5 + coord.1] != 0 {
            return Err("既に石があります");
        } else if 0 < self.players_has_piece[now_turn_player] {
            if self.is_there_surrounding_piece(ohajiki_num, coord) {
                return Err("周りに既に石があります");
            } else {
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
    fn flick_ohajiki(
        &mut self,
        coord: (usize, usize),
        angle: (isize, isize),
    ) -> Result<(), &'static str> {
        let now_turn_player = self.turn as usize;
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
                        self.prev_board = now_board.clone();
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
    fn winners(&self) -> Vec<bool> {
        let num_player = 2;
        let mut result: Vec<bool> = vec![false; num_player as usize];

        for i in 0..5 {
            for j in 0..5 {
                let idx = i + j * 5;
                let c = self.board_data[idx];
                if c == 0 {
                    continue;
                }

                if i < 5 - 2 {
                    if c == self.board_data[idx + 1] && c == self.board_data[idx + 2] {
                        result[c as usize - 1] = true;
                    }
                }
                if j < 5 - 2 {
                    if c == self.board_data[idx + 5] && c == self.board_data[idx + 10] {
                        result[c as usize - 1] = true;
                    }
                }
                if i < 5 - 2 && j < 5 - 2 {
                    if c == self.board_data[idx + 6] && c == self.board_data[idx + 12] {
                        result[c as usize - 1] = true;
                    }
                    if c == self.board_data[idx + 4] && c == self.board_data[idx + 8] {
                        result[c as usize - 1] = true;
                    }
                }
            }
        }
        result
    }
    fn _to_string(&self) -> String {
        let vidro = self;
        let mut buf = String::new();

        buf += "now turn player: ";
        buf += &(vidro.steps % (vidro.num_player as usize)).to_string();
        buf += "\n";

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
        hash += self.turn as u64;
        hash
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
            for i in 0..winners.len() {
                buf += &i.to_string();
                buf += &winners[i].to_string();
                buf += "\n";
            }
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

//ここから下は探索専用
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

fn win_eval_bit(vidro: &Vidro) -> Eval {
    let mut player_bits = [0u32; 2];

    for idx in 0..25 {
        let c = vidro.board_data[idx];
        if c != 0 {
            player_bits[c as usize - 1] |= 1 << idx;
        }
    }

    let mut result = [false; 2];
    for p in 0..2 {
        for &mask in WIN_MASKS.iter() {
            if player_bits[p] & mask == mask {
                result[p] = true;
                break;
            }
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

#[derive(Clone)]
struct Eval {
    value: EvalValue,
    evaluated: bool, //評価済みかどうか
}

fn is_board_reach(board: &Vidro) -> i8 {
    let mut vidro = board.clone();
    vidro.next_turn(); //故意に手番を書き換え2手差しさせたときに勝利することがあるかを調べる
    let children = create_children_on_node(&vidro, false);
    let turn = -(vidro.turn as i8) * 2 + 1;
    for child in &children {
        if let EvalValue::Win(value) = win_eval_bit_shift(child).value {
            if value == turn {
                return value;
            }
        }
    }
    return 0;
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

fn create_children_on_node(target_board: &Vidro, sorting: bool) -> Vec<Vidro> {
    let mut children: HashSet<Vidro> = HashSet::new();
    let mut new_vidro = target_board.clone();

    //可能な限りの子を作成
    for i in 0..5 {
        for j in 0..5 {
            if let Ok(()) = new_vidro.set_ohajiki((i, j)) {
                //テキトー置きが成功したとき
                //childrenに追加
                // let new_board = (new_vidro.board);
                canonical_board(&mut new_vidro.board_data);
                children.insert(new_vidro);
                new_vidro = target_board.clone(); //変更が加わってしまった盤面はもういらない。新しく作りなおす。
            }
        }
    }
    for i in 0..5 {
        for j in 0..5 {
            for a in 0..8 {
                if let Ok(()) = new_vidro.flick_ohajiki((i, j), ANGLES[a]) {
                    //テキトー置きが成功したとき
                    //childrenに追加
                    canonical_board(&mut new_vidro.board_data);
                    children.insert(new_vidro);
                    new_vidro = target_board.clone(); //変更が加わってしまった盤面はもういらない。新しく作りなおす。
                }
            }
        }
    }
    let mut result: Vec<Vidro> = children.into_iter().collect();
    if sorting {
        order_children(&mut result, target_board.turn);
    }
    result
}

fn alphabeta(
    board: &Vidro,
    depth: usize,
    alpha: i8,
    beta: i8,
    maximizing: bool,
    tt: &mut LruCache<u64, Eval>,
    route: &mut HashSet<Vidro>,
    process: &mut Progress,
) -> EvalValue {
    process.update(depth, tt.len(), board);

    //千日手判定
    if route.contains(&board) {
        return EvalValue::Draw; //引き分け評価
    }
    route.insert(board.clone());

    let hash = board.to_hash();

    if let Some(cached) = tt.get(&hash) {
        if cached.evaluated {
            match cached.value {
                EvalValue::Win(v) => {
                    route.remove(&board); // 探索パスから除去して戻る
                    return EvalValue::Win(v);
                }
                EvalValue::Draw => {
                    route.remove(&board); // 探索パスから除去して戻る
                    return EvalValue::Draw;
                }
                _ => (),
            }
        }
    }

    //自己評価
    let eval = win_eval_bit_shift(board);
    if eval.evaluated || depth == 0 {
        let eval = eval.value;
        tt.put(
            hash,
            Eval {
                value: eval.clone(),
                evaluated: true,
            },
        );

        route.remove(&board); // 探索パスから除去して戻る
        return eval;
    }

    let children = create_children_on_node(board, true);

    let mut alpha = alpha;
    let mut beta = beta;
    let mut value;

    let mut contains_unknown = false;

    if maximizing {
        value = i8::MIN;
        for child in &children {
            let score = match alphabeta(&child, depth - 1, alpha, beta, false, tt, route, process) {
                EvalValue::Win(score) => score,
                EvalValue::Draw => 0,
                EvalValue::Unknown => {
                    contains_unknown = true;
                    continue;
                }
            };
            value = value.max(score);
            alpha = alpha.max(value);
            if alpha >= beta {
                break;
            }
        }
    } else {
        value = i8::MAX;
        for child in &children {
            let score = match alphabeta(&child, depth - 1, alpha, beta, true, tt, route, process) {
                EvalValue::Win(score) => score,
                EvalValue::Draw => 0,
                EvalValue::Unknown => {
                    contains_unknown = true;
                    continue;
                }
            };
            value = value.min(score);
            beta = beta.min(value);
            if beta <= alpha {
                break;
            }
        }
    }

    let turn = -(board.turn as i8) * 2 + 1;

    let mut is_unknown = false;

    let this_node_eval = match value {
        i8::MIN => {
            is_unknown = true;
            EvalValue::Unknown
        }
        i8::MAX => {
            is_unknown = true;
            EvalValue::Unknown
        }
        _ => {
            if value == turn {
                EvalValue::Win(turn)
            } else if contains_unknown {
                is_unknown = true;
                EvalValue::Unknown
            } else if value == 0 {
                EvalValue::Draw
            } else {
                EvalValue::Win(-turn)
            }
        }
    };

    if !is_unknown {
        tt.put(
            hash,
            Eval {
                value: this_node_eval.clone(),
                evaluated: true,
            },
        );
    }

    route.remove(&board); // 探索パスから除去して戻る

    this_node_eval
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

    fn update(&mut self, current_depth: usize, tt_size: usize, board: &Vidro) {
        self.nodes_searched += 1;
        let now = Instant::now();
        if now.duration_since(self.last_print) >= Duration::from_secs(10) {
            println!(
                "探索ノード数: {}, 現在深さ: {}, TTサイズ: {}",
                self.nodes_searched, current_depth, tt_size
            );
            // println!("{}", board._to_string());
            self.last_print = now;
        }
    }
}

fn main() {
    // _play_vidro();
    // return;
    let capacity = NonZeroUsize::new(100_000).unwrap();
    let mut tt: LruCache<u64, Eval> = LruCache::new(capacity);

    let mut vidro = Vidro::new(0);

    // vidro.set_ohajiki((2, 2)).unwrap();
    // vidro.set_ohajiki((0, 0)).unwrap();
    // vidro.set_ohajiki((0, 4)).unwrap();
    // vidro.set_ohajiki((2, 0)).unwrap();
    // vidro.set_ohajiki((2, 4)).unwrap();

    // vidro.set_ohajiki((0, 0)).unwrap();
    // vidro.set_ohajiki((4, 4)).unwrap();
    // vidro.set_ohajiki((0, 2)).unwrap();
    // vidro.set_ohajiki((4, 2)).unwrap();
    // vidro.set_ohajiki((2, 1)).unwrap();
    // vidro.set_ohajiki((2, 3)).unwrap();

    let mut process = Progress::new();
    let mut route: HashSet<Vidro> = HashSet::new();
    let depth = 50;

    let mut result = EvalValue::Unknown;
    println!("depth: {}", 0);
    for depth_run in 1..=depth {
        result = alphabeta(
            &vidro,
            depth_run,
            i8::MIN,
            i8::MAX,
            true,
            &mut tt,
            &mut route,
            &mut process,
        );
        if let EvalValue::Win(_) = result {
            break;
        }
        route.clear(); //念のため
        println!("depth: {}", depth_run);
    }
    println!("評価値: {:?}", result);
}
