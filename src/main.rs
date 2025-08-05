use Vec;
use regex::Regex;
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

pub struct Vidro {
    board: [[u8; 5]; 5],
    steps: usize,
    prev_board: [[u8; 5]; 5],
    num_player: u8,
    players_has_piece: Vec<u8>,
}

impl Vidro {
    pub fn new(num_player: usize) -> Vidro {
        Vidro {
            board: [[0; 5]; 5],
            steps: 0,
            prev_board: [[0; 5]; 5],
            num_player: num_player.try_into().unwrap(),
            players_has_piece: vec![5; num_player],
        }
    }
    pub fn is_there_surrounding_piece(&self, ohajiki_num: u8, coord: (usize, usize)) -> bool {
        for i in 0..3 {
            for j in 0..3 {
                if coord.0 as isize + i - 1 < 0
                    || self.board.len() as isize <= coord.0 as isize + i - 1
                    || coord.1 as isize + j - 1 < 0
                    || self.board[0].len() as isize <= coord.1 as isize + j - 1
                {
                    continue;
                }
                if self.board[(coord.0 as isize + i) as usize - 1]
                    [(coord.1 as isize + j) as usize - 1]
                    == ohajiki_num
                {
                    return true;
                }
            }
        }
        false
    }
    pub fn set_ohajiki(&mut self, coord: (usize, usize)) -> Result<(), &'static str> {
        //プレイヤーについている数字+1をそのプレイヤーの石として設計している。
        let now_turn_player = self.steps % (self.num_player as usize);
        let ohajiki_num = (now_turn_player + 1).try_into().unwrap();

        if 0 < self.players_has_piece[now_turn_player] {
            if self.is_there_surrounding_piece(ohajiki_num, coord) {
                return Err("周りに既に石があります");
            } else {
                self.board[coord.0][coord.1] = ohajiki_num;
                self.players_has_piece[now_turn_player] -= 1;
                self.steps += 1;
                println!("{:?}", &self.board);
                return Ok(());
            }
        } else {
            return Err("もう置く石がありません");
        }
    }
    pub fn flick_ohajiki(
        &mut self,
        coord: (usize, usize),
        angle: (isize, isize),
    ) -> Result<(), &'static str> {
        let now_turn_player = self.steps % (self.num_player as usize);
        let ohajiki_num: u8 = (now_turn_player + 1).try_into().unwrap();

        let now_board = self.board.clone();

        let mut target = self.board[coord.0][coord.1];
        let mut target_coord: (isize, isize) = (coord.0 as isize, coord.1 as isize);

        if target != ohajiki_num {
            return Err("他人の駒をはじくことはできません");
        }

        let mut next: (isize, isize); //default 処理中での移動先の座標を示す。

        let mut roops = 0;

        while target != 0 {
            roops += 1;

            next = (target_coord.0 + angle.0, target_coord.1 + angle.1);

            if next.0 < 0
                || self.board.len() as isize <= next.0
                || next.1 < 0
                || self.board[0].len() as isize <= next.1
            {
                target = 0;
            } else {
                let u_next = (next.0 as usize, next.1 as usize);
                let u_target_coord = (target_coord.0 as usize, target_coord.1 as usize);

                match self.board[u_next.0][u_next.1] {
                    0 => {
                        //移動先に何もない場合
                        self.board[u_next.0][u_next.1] = target;
                        self.board[u_target_coord.0][u_target_coord.1] = 0;
                        target_coord = next;
                    }
                    _ => {
                        //移動先に駒がある場合
                        target_coord = next;
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
                for i in 0..self.board.len() {
                    for j in 0..self.board[0].len() {
                        if self.board[i][j] != now_board[i][j] {
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
            for i in 0..self.board.len() {
                for j in 0..self.board[0].len() {
                    if self.board[i][j] != self.prev_board[i][j] {
                        self.steps += 1;
                        return Ok(());
                    }
                }
            }
            //千日手の制約に引っかかる場合
            for i in 0..self.board.len() {
                //元の盤面に戻す
                for j in 0..self.board[0].len() {
                    self.board[i][j] = now_board[i][j];
                }
            }
            return Err("千日手です");
        }
    }
    fn winners(&self) -> Vec<bool> {
        let l1 = self.board.len();
        let l2 = self.board[0].len();
        let mut result: Vec<bool> = vec![false; self.num_player as usize];
        for i in 0..l1 {
            for j in 0..l2 {
                if i < l1 - 2 {
                    if self.board[i][j] == self.board[i + 1][j]
                        && self.board[i + 1][j] == self.board[i + 2][j]
                    {
                        result[self.board[i][j] as usize] = true;
                        continue;
                    }
                }
                if j < l2 - 2 {
                    if self.board[i][j] == self.board[i][j + 1]
                        && self.board[i][j + 1] == self.board[i][j + 2]
                    {
                        result[self.board[i][j] as usize] = true;
                        continue;
                    }
                }
                if i < l1 - 2 && j < l2 - 2 {
                    if self.board[i][j] == self.board[i + 1][j + 1]
                        && self.board[i + 1][j + 1] == self.board[i + 2][j + 2]
                    {
                        result[self.board[i][j] as usize] = true;
                        continue;
                    }
                    if self.board[i][j + 2] == self.board[i + 1][j + 1]
                        && self.board[i + 1][j + 1] == self.board[i + 2][j]
                    {
                        result[self.board[i][j] as usize] = true;
                        continue;
                    }
                }
            }
        }
        result
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
    let command_re = Regex::new(r"(set|flick)\s+(\d+)\/(\d+)(?:\s+(\d))?").unwrap();

    let mut read_buf = String::new();

    loop {
        buf.clear();

        //盤面作成
        buf += "now turn player: ";
        buf += &(vidro.steps % (vidro.num_player as usize)).to_string();
        buf += "\n";

        buf += "\u{001b}[47m  0 1 2 3 4\u{001b}[0m\n";
        for i in 0..vidro.board.len() {
            buf += "\u{001b}[47m";
            buf += &i.to_string();
            buf += "\u{001b}[0m";

            for j in 0..vidro.board[0].len() {
                buf += "\u{001b}[";
                buf += &(30 + vidro.board[i][j]).to_string();
                buf += if vidro.board[i][j] == 0 {
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
        buf += "\n input> ";

        println!("{}", buf);

        read_buf.clear();
        while read_buf.is_empty() {
            read_buf = read_buffer();

            match command_re.captures(&read_buf) {
                Some(caps) => {
                    let coord = (
                        caps[2].parse::<usize>().unwrap(),
                        caps[3].parse::<usize>().unwrap(),
                    );
                    let angle = caps[4].parse::<usize>();

                    match &caps[1] {
                        "set" => match vidro.set_ohajiki(coord) {
                            Ok(()) => (), //成功
                            Err(err) => {
                                println!("{}", err);
                                read_buf.clear();
                            }
                        },
                        "flick" => {
                            match angle {
                                Ok(angle) => {
                                    match vidro.flick_ohajiki(coord, ANGLES[angle % (8 as usize)]) {
                                        Ok(()) => (), //成功
                                        Err(err) => {
                                            println!("{}", err);
                                            read_buf.clear();
                                        }
                                    }
                                }
                                Err(_) => {
                                    println!("方向を入力してください");
                                    read_buf.clear();
                                }
                            }
                        }
                        &_ => {
                            println!(
                                "コマンドの読み取りに失敗しました。\ncommands:\n    set y/x\n    flick y/x angle"
                            );
                            read_buf.clear();
                        }
                    }
                }
                None => {
                    println!(
                        "コマンドの読み取りに失敗しました。\ncommands:\n    set y/x\n    flick y/x angle"
                    );
                    read_buf.clear();
                }
            }
        }
    }
}

fn main() {
    play_vidro();
}
