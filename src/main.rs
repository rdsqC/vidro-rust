use Vec;
use lru::LruCache;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::atomic::AtomicUsize;
use std::time::{Duration, Instant};
use std::{io, result};

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
    fn replace(&mut self, board: u64) {
        *self = Self::new(board);
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

        if 0 < self.players_has_piece[now_turn_player] {
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

//ここから下は探索専用
fn win_eval(vidro: &Vidro) -> Eval {
    let num_player = 2;
    let mut result: Vec<bool> = vec![false; num_player as usize];

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

fn canonical_board(board: &mut [u8; 25]) {
    for t in 1..8 {
        //0の場合は無変更になるため実行しない
        let mut transformed = apply_transfrom(&board, t);
        if board.cmp(&&mut transformed).is_lt() {
            board.copy_from_slice(&transformed);
        };
    }
}

fn apply_transfrom(board_data: &[u8; 25], t: u8) -> [u8; 25] {
    let mut result = [0u8; 25];
    for v1 in 0..5 {
        for v2 in 0..5 {
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
        }
    }
    result
}

const DONT_HAS_PARENT: u64 = u64::MAX; //Nodeにおいて親を持たないことを示す特殊値とする

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

struct Node {
    eval: Eval,
    parent: u64,
    children: Vec<u64>,
    num_searchs: usize,
}

impl Node {
    pub fn new(parent: u64) -> Self {
        Node {
            eval: Eval {
                value: EvalValue::Unknown,
                evaluated: false,
            },
            parent: parent,
            children: vec![],
            num_searchs: 0,
        }
    }
    fn is_root(&self) -> bool {
        self.parent == DONT_HAS_PARENT
    }
}

fn board_turn(board: u64) -> i8 {
    (board & 0b11) as i8 * (-2) + 1
}

fn is_board_reach(board: &Vidro) -> i8 {
    let mut vidro = board.clone();
    vidro.next_turn(); //故意に手番を書き換え2手差しさせたときに勝利することがあるかを調べる
    let children = create_children_on_node(&vidro, false);
    let turn = -(vidro.turn as i8) * 2 + 1;
    for child in &children {
        if let EvalValue::Win(value) = win_eval(child).value {
            if value == turn {
                return value;
            }
        }
    }
    return 0;
}

fn quick_eval(board: &Vidro) -> i8 {
    is_board_reach(board)
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
                    // let new_board = (new_vidro.board);
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

fn get_or_insert(tt: &mut LruCache<Vidro, Node>, board: Vidro) -> &mut Node {
    if tt.contains(&board) {
        tt.get_mut(&board).unwrap()
    } else {
        tt.put(board.clone(), Node::new(DONT_HAS_PARENT));
        tt.get_mut(&board).unwrap()
    }
}

// fn run_search(board: u64, depth: usize) -> EvalValue {
//     let (alpha, beta) = (i8::MIN, i8::MAX);
//     let maximizing = board & 0b1 == 0;
//     let capacity = NonZeroUsize::new(1_000_000).unwrap();
//     let mut tt: LruCache<Board, Node> = LruCache::new(capacity);
//     let mut route: HashSet<u64> = HashSet::new();
//     alphabeta(board, depth, alpha, beta, maximizing, &mut tt, &mut route)
// }

fn alphabeta(
    board: &Vidro,
    depth: usize,
    alpha: i8,
    beta: i8,
    maximizing: bool,
    tt: &mut LruCache<Vidro, Node>,
    route: &mut HashSet<Vidro>,
    process: &mut Progress,
) -> EvalValue {
    process.update(depth, tt.len());

    //千日手判定
    if route.contains(&board) {
        return EvalValue::Draw; //引き分け評価
    }
    route.insert(board.clone());

    if let Some(cached) = tt.get(board) {
        if cached.eval.evaluated {
            match cached.eval.value {
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
    let eval = win_eval(board);
    if eval.evaluated || depth == 0 {
        let eval = eval.value;
        get_or_insert(tt, board.clone()).eval = Eval {
            value: eval.clone(),
            evaluated: true,
        };

        route.remove(&board); // 探索パスから除去して戻る
        return eval;
    }

    let children = create_children_on_node(board, false);

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
        let node = get_or_insert(tt, board.clone());
        node.eval = Eval {
            value: this_node_eval.clone(),
            evaluated: true,
        };
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

    fn update(&mut self, current_depth: usize, tt_size: usize) {
        self.nodes_searched += 1;
        let now = Instant::now();
        if now.duration_since(self.last_print) >= Duration::from_secs(10) {
            println!(
                "探索ノード数: {}, 現在深さ: {}, TTサイズ: {}",
                self.nodes_searched, current_depth, tt_size
            );
            self.last_print = now;
        }
    }
}

type Board = u64;

fn main() {
    // _play_vidro();
    // return;
    // println!("aaii");
    // let num_nodes = {
    //     let a: usize = 2;
    //     a.pow(20)
    // };
    // let result = research(vidro.board, num_nodes);
    // println!("result: {:?}", result);
    let capacity = NonZeroUsize::new(1_000_000).unwrap();
    let mut tt: LruCache<Vidro, Node> = LruCache::new(capacity);
    // let mut tt: HashMap<u64, Node> = HashMap::new();

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
    for depth_run in 1..depth {
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
