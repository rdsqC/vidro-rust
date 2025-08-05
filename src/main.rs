use Vec;

const ANGLES: [(isize, isize); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
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
        let now_board = self.board.clone();

        let mut target = self.board[coord.0][coord.1];
        let mut target_coord: (isize, isize) = (coord.0 as isize, coord.1 as isize);

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
            for i in 0..self.board.len() {
                //千日手の防止
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

fn main() {}
