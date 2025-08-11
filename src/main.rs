use Vec;
use regex::Regex;
use std::collections::{BTreeMap, HashMap};
use std::io;

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

#[derive(Clone)]
pub struct Vidro {
    board: u64,
    steps: usize,
    prev_board: u64,
    num_player: u8,
    players_has_piece: [u8; 2],
}

impl Vidro {
    pub fn new(board: u64) -> Vidro {
        let mut board = board;
        let mut players_has_piece = [5; 2];
        for _ in 0..25 {
            board >>= 2;
            match board & 0b11 {
                0b01 => players_has_piece[0] -= 1,
                0b10 => players_has_piece[1] -= 1,
                _ => (),
            }
        }
        Vidro {
            board: board,
            steps: board as usize & 0b11,
            prev_board: 0,
            num_player: 2, //強制的2人プレイ
            players_has_piece: players_has_piece,
        }
    }
    fn replace(&mut self, board: u64) {
        *self = Self::new(board);
    }
    fn get_trout(&self, v1: usize, v2: usize) -> u64 {
        let result = self.board;
        result >> 2 >> 2 * (24 - (5 * v1 + v2)) & 0b11
    }
    fn set_trout(&mut self, v1: usize, v2: usize, value: u64) {
        let mask: u64 = !(0b11 << 2 << 2 * (24 - (5 * v1 + v2)));
        let set_bit = (value & 0b11) << 2 << 2 * (24 - (5 * v1 + v2));
        self.board &= mask;
        self.board |= set_bit;
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
        self.board = Self::set_hash_turn(self.board, turn);
    }
    fn is_there_surrounding_piece(&self, ohajiki_num: u64, coord: (usize, usize)) -> bool {
        for i in 0..3 {
            for j in 0..3 {
                if coord.0 as isize + i - 1 < 0
                    || 5 as isize <= coord.0 as isize + i - 1
                    || coord.1 as isize + j - 1 < 0
                    || 5 as isize <= coord.1 as isize + j - 1
                {
                    continue;
                }
                if self.get_trout(
                    (coord.0 as isize + i) as usize - 1,
                    (coord.1 as isize + j) as usize - 1,
                ) == ohajiki_num
                {
                    return true;
                }
            }
        }
        false
    }
    fn set_ohajiki(&mut self, coord: (usize, usize)) -> Result<(), &'static str> {
        //プレイヤーについている数字+1をそのプレイヤーの石として設計している。
        let now_turn_player = self.get_now_turn() as usize;
        let ohajiki_num = (now_turn_player + 1).try_into().unwrap();

        if 0 < self.players_has_piece[now_turn_player] {
            if self.is_there_surrounding_piece(ohajiki_num, coord) {
                return Err("周りに既に石があります");
            } else {
                self.set_trout(coord.0, coord.1, ohajiki_num);
                self.players_has_piece[now_turn_player] -= 1;
                self.steps += 1;
                self.set_turn(now_turn_player as u8);
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
        let now_turn_player = self.get_now_turn() as usize;
        let ohajiki_num: u64 = (now_turn_player + 1).try_into().unwrap();

        let now_board = self.board.clone();

        let mut target = self.get_trout(coord.0, coord.1);
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

                match self.get_trout(u_next.0, u_next.1) {
                    0 => {
                        //移動先に何もない場合
                        self.set_trout(u_next.0, u_next.1, target);
                        self.set_trout(u_target_coord.0, u_target_coord.1, 0);
                        target_coord = next;
                    }
                    _ => {
                        //移動先に駒がある場合
                        target_coord = next;
                        target = self.get_trout(u_next.0, u_next.1);
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
                        if self.get_trout(i, j) != Vidro::get_hash_trout(now_board, i, j) {
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
                    if self.get_trout(i, j) != Vidro::get_hash_trout(self.prev_board, i, j) {
                        self.steps += 1;

                        self.set_turn(now_turn_player as u8);
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
                    self.set_trout(i, j, Vidro::get_hash_trout(now_board, i, j));
                }
            }
            return Err("千日手です");
        }
    }
    fn winners(&self) -> Vec<bool> {
        let l1 = 5;
        let l2 = 5;
        let mut result: Vec<bool> = vec![false; self.num_player as usize];
        for i in 0..l1 {
            for j in 0..l2 {
                if i < l1 - 2 {
                    if self.get_trout(i, j) == self.get_trout(i + 1, j)
                        && self.get_trout(i + 1, j) == self.get_trout(i + 2, j)
                    {
                        if 0 < self.get_trout(i, j) as usize {
                            result[self.get_trout(i, j) as usize - 1] = true;
                        }
                    }
                }
                if j < l2 - 2 {
                    if self.get_trout(i, j) == self.get_trout(i, j + 1)
                        && self.get_trout(i, j + 1) == self.get_trout(i, j + 2)
                    {
                        if 0 < self.get_trout(i, j) as usize {
                            result[self.get_trout(i, j) as usize - 1] = true;
                        }
                    }
                }
                if i < l1 - 2 && j < l2 - 2 {
                    if self.get_trout(i, j) == self.get_trout(i + 1, j + 1)
                        && self.get_trout(i + 1, j + 1) == self.get_trout(i + 2, j + 2)
                    {
                        if 0 < self.get_trout(i, j) as usize {
                            result[self.get_trout(i, j) as usize - 1] = true;
                        }
                    }
                    if self.get_trout(i, j + 2) == self.get_trout(i + 1, j + 1)
                        && self.get_trout(i + 1, j + 1) == self.get_trout(i + 2, j)
                    {
                        if 0 < self.get_trout(i, j + 2) as usize {
                            result[self.get_trout(i, j + 2) as usize - 1] = true;
                        }
                    }
                }
            }
        }
        result
    }
    fn get_now_turn(&self) -> u8 {
        return self.steps as u8 % self.num_player;
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

fn play_vidro() {
    let mut vidro = Vidro::new(2);
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
                buf += &(30 + vidro.get_trout(i, j)).to_string();
                buf += if vidro.get_trout(i, j) == 0 {
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
fn hash_board(board: &Vidro) -> u64 {
    let mut hash: u64 = 0;
    for i in 0..5 {
        for j in 0..5 {
            hash <<= 2;
            hash |= (board.get_trout(i, j) as u64) & 0b11;
        }
    }
    hash <<= 2;
    hash |= (board.get_now_turn() as u64) & 0b11; //手番もまぜる
    hash
}

fn win_eval(hash: u64) -> (i8, bool) {
    let l1 = 5;
    let l2 = 5;
    let num_player = 2;
    let mut result: Vec<bool> = vec![false; num_player as usize];

    let board = |v1: usize, v2: usize| -> u64 {
        let result = hash;
        result >> 2 >> 2 * (24 - (5 * v1 + v2)) & 0b11
    };

    for i in 0..l1 {
        for j in 0..l2 {
            if i < l1 - 2 {
                if board(i, j) == board(i + 1, j) && board(i + 1, j) == board(i + 2, j) {
                    if 0 < board(i, j) as usize {
                        result[board(i, j) as usize - 1] = true;
                    }
                }
            }
            if j < l2 - 2 {
                if board(i, j) == board(i, j + 1) && board(i, j + 1) == board(i, j + 2) {
                    if 0 < board(i, j) as usize {
                        result[board(i, j) as usize - 1] = true;
                    }
                }
            }
            if i < l1 - 2 && j < l2 - 2 {
                if board(i, j) == board(i + 1, j + 1) && board(i + 1, j + 1) == board(i + 2, j + 2)
                {
                    if 0 < board(i, j) as usize {
                        result[board(i, j) as usize - 1] = true;
                    }
                }
                if board(i, j + 2) == board(i + 1, j + 1) && board(i + 1, j + 1) == board(i + 2, j)
                {
                    if 0 < board(i, j + 2) as usize {
                        result[board(i, j + 2) as usize - 1] = true;
                    }
                }
            }
        }
    }

    let eval: i8 = if result[0] { 1 } else { 0 } + if result[1] { -1 } else { 0 };
    (eval, result[0] || result[1])
}

const DONT_HAS_PARENT: u64 = u64::MAX; //Nodeにおいて親を持たないことを示す特殊値とする

struct Eval {
    value: i8,
    evaluated: bool,
}

struct Node {
    eval: Eval,
    parent: u64,
    children: Vec<u64>,
}

impl Node {
    pub fn new(parent: u64) -> Self {
        Node {
            eval: Eval {
                value: 0,
                evaluated: false,
            },
            parent: parent,
            children: vec![],
        }
    }
    fn is_root(&self) -> bool {
        self.parent == DONT_HAS_PARENT
    }
}

fn research(board: u64, deeps: usize) -> bool {
    let root_node = Node::new(DONT_HAS_PARENT);

    let mut tt: HashMap<u64, Node> = HashMap::new();
    tt.insert(board, root_node);
    let mut target_board = board;

    let mut new_vidro = Vidro::new(target_board);

    for deep in 0..deeps {
        //可能な限りの子を作成

        //targetのnodeをttから取得しておく
        let Some(target_node) = tt.get_mut(&target_board) else {
            continue;
        };
        for i in 0..5 {
            for j in 0..5 {
                if let Ok(()) = new_vidro.set_ohajiki((i, j)) {
                    //テキトー置きが成功したとき
                    //childrenに追加
                    target_node.children.push(new_vidro.board);
                    new_vidro.replace(target_board); //変更が加わってしまった盤面はもういらない。新しく作りなおす。
                }
                for a in 0..8 {
                    if let Ok(()) = new_vidro.flick_ohajiki((i, j), ANGLES[a]) {
                        //テキトー置きが成功したとき
                        //childrenに追加
                        target_node.children.push(new_vidro.board);
                        new_vidro.replace(target_board); //変更が加わってしまった盤面はもういらない。新しく作りなおす。
                    }
                }
            }
        }
    }

    return false;

    // let tt: &mut HashMap<u64, (i8, bool)>,
}

fn main() {
    // let mut vidro = Vidro::new(2);
    research(vidro.board, 10);
    // vidro.set_ohajiki((0, 0)).unwrap();
    // vidro.set_ohajiki((4, 0)).unwrap();
    // vidro.set_ohajiki((0, 2)).unwrap();
    // vidro.flick_ohajiki((4, 0), (0, 1)).unwrap();
    //
    // let mut tt: HashMap<u64, (i8, bool)> = HashMap::new(); // 1 先手勝利, -1 後手勝利, 0 引き分け, ふたつめには評価済みかどうかの真偽値
    // println!("result evaluation: {:?}", research(&mut tt, 24));
}
