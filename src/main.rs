mod bitboard;
mod bitboard_console;
mod eval_value;
use Vec;
use bitboard::{Bitboard, FIELD_BOD, FIELD_BOD_HEIGHT, FIELD_BOD_WIDTH, MoveBit};
use bitboard_console::BitboardConsole;
use eval_value::{Eval, EvalValue};
use lru::LruCache;
use rand::seq::SliceRandom;
use rand::thread_rng;
use regex::Regex;
use std::fs::{File, OpenOptions, metadata};
use std::io::Write;
use std::num::NonZeroUsize;
use std::os::unix::raw::pid_t;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::{io, usize};

//ここから下は探索専用

fn static_evaluation(vidro: &mut Bitboard, prev_move: Option<MoveBit>) -> i16 {
    let threats = evaluate_threats(&vidro);
    let have_piece = evaluate_have_piece(&vidro);
    let position = evaluate_position(&vidro);
    let reach = evaluate_reach(vidro, prev_move);

    // 自分の「詰めろ」になる手の数が多いほど、局面は有利
    let my_threats = generate_threat_moves(vidro, prev_move).len() as i16;
    let threat_score = my_threats * 25; // 例：1つの脅威手を25点と評価

    // 相手の脅威の数も計算し、評価値から引くとなお良い
    vidro.turn_change();
    let opponent_threats = generate_threat_moves(vidro, prev_move).len() as i16;
    let opponent_threat_score = opponent_threats * 25;
    vidro.turn_change(); // ★手番を必ず元に戻す

    threats + have_piece * 100 + position + threat_score - opponent_threat_score
}

fn evaluate_position(vidro: &Bitboard) -> i16 {
    let mut score = 0;
    let mut players_bod = vidro.player_bods;
    for p in 0..2 {
        let p_turn = -(p as i16 * 2) + 1;
        while players_bod[p] != 0 {
            let mut idx = players_bod[p].trailing_zeros() as usize;
            idx = idx / 5 + idx % FIELD_BOD_WIDTH as usize;
            score += POSITION_SCORES[idx] * p_turn;
            players_bod[p] &= players_bod[p] - 1;
        }
    }
    score += (vidro.can_set_count(0) as i16 - vidro.can_set_count(1) as i16) * 10;
    score
}

const POSITION_SCORES: [i16; 25] = [
    10, 0, 9, 0, 10, //
    0, 2, 4, 2, 0, //
    9, 4, 12, 4, 9, //中央 > 角 > 辺の中央 > その他
    0, 2, 4, 2, 0, //
    10, 0, 9, 0, 10, //
];

fn evaluate_have_piece(vidro: &Bitboard) -> i16 {
    vidro.have_piece[1] as i16 - vidro.have_piece[0] as i16
}

fn evaluate_threats(vidro: &Bitboard) -> i16 {
    const OPEN_TWO_SCORE: i16 = 0; // _XX_ (両側が空いている2)
    const SEMI_OPEN_TWO_SCORE: i16 = 300; // OXX_ や _XXO (片側が空いている2)
    const SEMI_OPEN_SPLIT_ONE_SCORE: i16 = 300; //X_X (1つ空きオープン)
    const OPEN_SPLIT_ONE_SCORE: i16 = 0; //上のX_Xに含まれる _X_X_ (1つ空きのオープンな2)

    const MARGIN_WIDTH: u64 = 9;

    let blank: u64 = FIELD_BOD & !(vidro.player_bods[0] | vidro.player_bods[1]); //空白マス

    let mut total_score = 0i16;

    for p in 0..2 {
        let me = vidro.player_bods[p];
        let opp = vidro.player_bods[1 - p];
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
            let pattern_open_two = (blank >> 0) & (me >> d) & (me >> (d * 2)) & (blank >> (d * 3));
            player_score += pattern_open_two.count_ones() as i16 * OPEN_TWO_SCORE;

            // パターン2: 片側が空いた2 (OXX_)
            // パターン: [相手, 自分, 自分, 空き]
            let pattern_semi_open_two_a =
                (opp >> 0) & (me >> d) & (me >> (d * 2)) & (blank >> (d * 3));
            player_score += pattern_semi_open_two_a.count_ones() as i16 * SEMI_OPEN_TWO_SCORE;

            // パターン3: 片側が空いた2 (_XXO)
            // パターン: [空き, 自分, 自分, 相手]
            let pattern_semi_open_two_b =
                (blank >> 0) & (me >> d) & (me >> (d * 2)) & (opp >> (d * 3));
            player_score += pattern_semi_open_two_b.count_ones() as i16 * SEMI_OPEN_TWO_SCORE;

            // パターン4: 1つ空きのオープンな2 (_X_X_)
            // パターン: [空き, 自分, 空き, 自分, 空き]
            let pattern_open_split_one = (blank >> 0)
                & (me >> d)
                & (blank >> (d * 2))
                & (me >> (d * 3))
                & (blank >> (d * 4));
            player_score += pattern_open_split_one.count_ones() as i16 * OPEN_SPLIT_ONE_SCORE;

            let pattern_semi_open_split_one = me & (blank >> d) & (me >> (d * 2));
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

fn evaluate_reach(vidro: &mut Bitboard, prev_move: Option<MoveBit>) -> i16 {
    vidro.turn_change(); //意図的に手番を書き換え2手差しさせたときに勝利することがあるかを調べる
    let moves = vidro.generate_legal_move(prev_move);
    let turn = vidro.turn;
    for &mv in &moves {
        vidro.apply_force(mv);
        if let EvalValue::Win(value) = vidro.win_eval().value {
            if value as i8 == turn {
                vidro.undo_force(mv);
                vidro.turn_change();
                return value as i16 * (10 - vidro.have_piece[0] - vidro.have_piece[1]) as i16 * 15;
            }
        }
        vidro.undo_force(mv);
    }
    vidro.turn_change();
    return 0;
}

fn find_mate_in_one_move(vidro: &mut Bitboard) -> Option<MoveBit> {
    let moves = vidro.generate_legal_move(None); //詰み探索には千日手を除くことはしなくてよい。積んでいる局面になったときに千日手盤面になることはないため
    let turn = vidro.turn;
    for &mv in &moves {
        vidro.apply_force(mv);
        if let EvalValue::Win(value) = vidro.win_eval().value {
            if value as i8 == turn {
                vidro.undo_force(mv);
                return Some(mv);
            }
        }
        vidro.undo_force(mv);
    }
    None
}

//先後最善を指した時の詰み手順
fn find_mate_sequence(
    vidro: &mut Bitboard,
    max_depth: usize,
    prev_move: Option<MoveBit>,
) -> Option<Vec<MoveBit>> {
    if vidro.have_piece[((-vidro.turn + 1) / 2) as usize] > 2 {
        return None;
    }

    let mut sequence = Vec::new();
    //詰みがあるかどうかをしらべてある場合は手順を構築する
    let result =
        find_mate_sequence_recursive(vidro, max_depth, usize::MIN, usize::MAX, true, prev_move);

    if let Some((_, first_move)) = result {
        //手順構築
        sequence.push(first_move);

        vidro.apply_force(first_move);

        let mut idx = 0;
        while !vidro.win_eval().evaluated {
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
                Some(first_move),
            ) {
                vidro.apply_force(best_next_move);
                sequence.push(best_next_move);
            } else {
                //手順が見つからなかった(バグの可能性が高い)
                for &mv in sequence.iter().rev() {
                    vidro.undo_force(mv);
                }
                return None;
            }
            idx += 1;
        }

        //見つかった手順を使って盤面を呼び出し前の状態に復元
        for &mv in sequence.iter().rev() {
            vidro.undo_force(mv);
        }

        Some(sequence)
    } else {
        None
    }
}

fn find_mate_sequence_recursive(
    vidro: &mut Bitboard,
    depth: usize,
    alpha: usize,
    beta: usize,
    is_attacker: bool,
    prev_move: Option<MoveBit>,
) -> Option<(usize, MoveBit)> {
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
        let attacking_moves = vidro.generate_legal_move(prev_move);
        if attacking_moves.is_empty() {
            return None;
        }

        let mut max_depth_found = usize::MIN; //最終的な詰みの深さ
        let mut best_move: Option<MoveBit> = None;

        for mv in attacking_moves {
            vidro.apply_force(mv);
            let result =
                find_mate_sequence_recursive(vidro, depth - 1, alpha, beta, false, Some(mv));
            vidro.undo_force(mv); //ミスを防ぐためにすぐ戻す
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
        let defending_moves = vidro.generate_legal_move(prev_move);
        //合法手が一つもないということは起きないため空の場合は考えない
        // if defending_moves.is_empty() {
        // return Some(depth + 1);
        // }

        let mut min_depth_found = usize::MAX;
        let mut best_move: Option<MoveBit> = None;

        //相手の手番で再帰呼び出し
        for mv in defending_moves {
            vidro.apply_force(mv);
            let result =
                find_mate_sequence_recursive(vidro, depth - 1, alpha, beta, true, Some(mv));
            vidro.undo_force(mv);

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
fn find_mate(
    vidro: &mut Bitboard,
    max_depth: usize,
    prev_move: Option<MoveBit>,
) -> Option<MoveBit> {
    let mut mate_move = MoveBit::from_idx(0, 8);
    if find_mate_recursive(vidro, max_depth, &mut mate_move, prev_move) {
        Some(mate_move)
    } else {
        None
    }
}

// 詰み探索の本体（再帰関数）
fn find_mate_recursive(
    vidro: &mut Bitboard,
    depth: usize,
    mate_move: &mut MoveBit,
    prev_move: Option<MoveBit>,
) -> bool {
    //深さ切れ(詰みなしと判断)
    if depth == 0 {
        return false;
    }

    let attacking_moves = generate_threat_moves(vidro, prev_move);
    if attacking_moves.is_empty() {
        return false; //詰めろを掛けられない
    }

    //OR探索
    for mv in attacking_moves {
        vidro.apply_force(mv);

        //受けが無くなっているかどうかを調べる
        if check_opponent_defense(vidro, depth - 1, mate_move, Some(mv)) {
            //受けがないことが確定 == 詰みが見つかった
            vidro.undo_force(mv);
            *mate_move = mv.clone(); //最後の代入の値==最初に指す手==詰み手順に入る時の手
            return true;
        }
        vidro.undo_force(mv);
    }

    //どの手も詰みにならなかった
    false
}

//NOTE! 詰みの読み筋を相手の物も含めるようにする

//受けがないかどうか
fn check_opponent_defense(
    vidro: &mut Bitboard,
    depth: usize,
    mate_move: &mut MoveBit,
    prev_move: Option<MoveBit>,
) -> bool {
    //勝になっていないかを確認
    if let EvalValue::Win(v) = vidro.win_eval().value {
        if v as i8 == -vidro.turn {
            return true;
        }
    }

    if depth == 0 {
        return false;
    }

    let defending_moves = vidro.generate_legal_move(prev_move);
    if defending_moves.is_empty() {
        //受けなし
        return true;
    }

    // 生成した受け手の全ての応手に対して、詰み手順が続くか調べる (AND検索)
    for mv in defending_moves {
        vidro.apply_force(mv);

        // 自分が再度攻めて詰むかどうかを再帰的に調べる
        let can_mate = find_mate_recursive(vidro, depth - 1, mate_move, Some(mv));

        vidro.undo_force(mv);

        if !can_mate {
            // 相手のこの受けで詰みが途切れた。
            // したがって、元の自分の手は必勝の詰み手順ではない。
            return false;
        }
    }

    // 相手がどう受けても、全て詰み手順が続くことが証明された
    true
}

fn is_reach(vidro: &mut Bitboard) -> bool {
    vidro.turn_change(); //意図的に手番を書き換え2手差しさせたときに勝利することがあるかを調べる
    let result = checkmate_in_one_move(vidro);
    vidro.turn_change(); //手番を戻す
    result
}

fn checkmate_in_one_move(vidro: &mut Bitboard) -> bool {
    let moves = vidro.generate_legal_move_only_flick(None);
    let turn = vidro.turn;
    for &mv in &moves {
        vidro.apply_force(mv);
        if let EvalValue::Win(value) = vidro.win_eval().value {
            if value as i8 == turn {
                vidro.undo_force(mv);
                return true;
            }
        }
        vidro.undo_force(mv);
    }
    false
}

fn generate_threat_moves(vidro: &mut Bitboard, prev_move: Option<MoveBit>) -> Vec<MoveBit> {
    let mut moves = vidro.generate_legal_move(prev_move);
    moves.retain(|&mv| {
        vidro.apply_force(mv);
        //詰めろ(自殺手を除く)
        if is_reach(vidro) && !checkmate_in_one_move(vidro) {
            vidro.undo_force(mv);
            return true;
        }
        vidro.undo_force(mv);
        false
    });
    moves
}

fn evaluate_for_negamax(board: &mut Bitboard, prev_move: Option<MoveBit>) -> i16 {
    // eval_mon(board, prev_move)
    static_evaluation(board, prev_move) * board.turn as i16
}

#[derive(Clone, Default)]
pub struct SearchInfo {
    pub depth: usize,
    pub score: i16,
    pub pv: Vec<MoveBit>,
    pub nodes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TTFlag {
    Exact,      // このスコアは真の評価値 (alpha < score < beta)
    LowerBound, // このスコアは下限値 (score >= beta, betaカットで得られた)
    UpperBound, // このBitboardは上限値 (score <= alpha, 有望な手が見つからなかった)
}

// 置換表に保存するデータ構造
#[derive(Clone, Copy, Debug)]
struct TTEntry {
    score: i16,
    depth: u8, // 保存したときの探索深さ
    flag: TTFlag,
    best_move: MoveBit, // その局面で見つかった最善手
}

const USE_CACHE: bool = false;
const USE_CACHE_DEPTH: usize = 8;

const DRAW_SCORE: i16 = 0;
const WIN_LOSE_SCORE: i16 = 30000;

fn alphabeta(
    board: &mut Bitboard,
    depth: usize,
    mut alpha: i16,
    mut beta: i16,
    tt: &mut LruCache<u64, TTEntry>,
    route: &mut Vec<u64>,
    process: &mut Progress,
    is_root: bool, // ★自分がルートノード（探索の起点）かを知るためのフラグ
    shared_info: Arc<Mutex<SearchInfo>>, // ★情報共有のための構造体
    log_file: &Arc<Mutex<File>>,
    prev_move: Option<MoveBit>,
) -> (i16, Vec<MoveBit>) {
    process.update(depth, board, tt.len());
    let mut best_pv = Vec::new();
    // canonical_board(&mut canonical_board_data);
    let hash = board.to_small_bod();
    //千日手判定
    if route.contains(&hash) {
        return (DRAW_SCORE, Vec::new()); //引き分け評価
    }
    route.push(hash);

    //自己評価
    let terminal_eval = board.win_eval();
    if terminal_eval.evaluated {
        route.pop();
        let score = if let EvalValue::Win(winner) = terminal_eval.value {
            if winner as i8 == board.turn {
                WIN_LOSE_SCORE
            } else {
                -WIN_LOSE_SCORE
            }
        } else {
            DRAW_SCORE
        };
        return (score, Vec::new());
    }

    if depth == 0 {
        route.pop();
        let tsumi_result = find_mate_sequence(board, 5, prev_move);

        let static_score = evaluate_for_negamax(board, prev_move);
        //詰み探索を実行
        let tsumi_found = if tsumi_result.is_some() { 1 } else { 0 };

        let log_line = format!("{},{}", tsumi_found, board.to_small_bod());

        //ファイル出力
        writeln!(log_file.lock().unwrap(), "{}", log_line).expect("ログを書き込めませんでした");

        if let Some(mate_sequence) = tsumi_result {
            // board.print_data();
            // 詰みを発見！スコアを「勝ち」に格上げし、手順も返す
            let mate_score = WIN_LOSE_SCORE - mate_sequence.len() as i16;
            return (mate_score, mate_sequence);
        }
        return (static_score, best_pv);
    }

    let original_alpha = alpha;
    let mut best_move_from_tt: Option<MoveBit> = None;
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

    let mut moves = board.generate_legal_move(prev_move);
    if let Some(tt_move) = best_move_from_tt {
        if let Some(pos) = moves.iter().position(|m| *m == tt_move) {
            let m = moves.remove(pos);
            moves.insert(0, m);
        }
    }

    let mut best_score = i16::MIN;
    let mut best_move: Option<MoveBit> = None;

    for &mv in &moves {
        //手を実行
        board.apply_force(mv);
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
            log_file,
            Some(mv),
        );
        score = -score;
        board.undo_force(mv); //Bitboardに戻す
        if best_score < score {
            best_score = score;
            best_move = Some(mv);
            best_pv.clear();
            best_pv.push(mv);
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

    if 29900 < best_score {
        let log_line = format!("{},{}", 1, board.to_small_bod());
        //ファイル出力
        writeln!(log_file.lock().unwrap(), "{}", log_line).expect("ログを書き込めませんでした");
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

    fn update(&mut self, current_depth: usize, board: &Bitboard, tt_len: usize) {
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

fn find_best_move(
    board: &mut Bitboard,
    max_depth: usize,
    tt: Arc<Mutex<LruCache<u64, TTEntry>>>,
    log_file: Arc<Mutex<File>>,
    prev_move: Option<MoveBit>,
) -> Option<MoveBit> {
    // if let Some(mate_sequence) = find_mate_sequence(board, 15) {
    //     // 15手詰みを探す
    //     println!("*** 詰み手順発見！ 初手: {:?} ***", mate_sequence[0]);
    //     return Some(mate_sequence[0].clone());
    // }

    let shared_info = Arc::new(Mutex::new(SearchInfo::default()));
    let info_clone_for_ui = shared_info.clone();

    let mut route: Vec<u64> = Vec::new();

    let mut best_move: Option<MoveBit> = None;

    let tt_for_thread = Arc::clone(&tt);
    let mut vidro_for_search = board.clone();

    let search_thread = thread::spawn(move || {
        let mut tt_guard = tt_for_thread.lock().unwrap();
        let mut process = Progress::new();

        let mut best_move_overall: Option<MoveBit> = None;

        //反復深化ループ
        for depth_run in 0..=max_depth {
            let mut route = Vec::new();

            //ルートノードで探索
            let (score, pv_sequence) = alphabeta(
                &mut vidro_for_search,
                depth_run,
                i16::MIN + 1,
                i16::MAX,
                &mut tt_guard,
                &mut route,
                &mut process,
                true,
                shared_info.clone(),
                &log_file,
                prev_move,
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
    search_thread.join().unwrap()
}

fn eval_mon(bit_vidro: &mut Bitboard, prev_move: Option<MoveBit>) -> i16 {
    const N: usize = 100;
    const MAX_EVAL: i16 = 1000;
    let mut eval = 0;
    for _ in 0..N {
        let mut moves = Vec::new();
        let mut prev_move_for_eval = prev_move;
        let mut win_eval_result = Eval {
            evaluated: false,
            value: EvalValue::Unknown,
        };
        while !win_eval_result.evaluated {
            // println!("{}", bit_vidro.to_string());
            // bit_vidro.print_data();
            let legal_move = bit_vidro.generate_legal_move(prev_move_for_eval);
            // MoveBit::print_vec_to_string(&legal_move);
            // MoveBit::print_vec_to_string(&bit_vidro.generate_maybe_threat_moves(prev_move_for_eval));
            let mut rng = thread_rng();
            // let mv = Bitboard::read_to_move();
            let mv = legal_move.choose(&mut rng).unwrap();
            // println!("決定手: {}", mv.to_string());
            bit_vidro.apply_force(*mv);
            moves.push(*mv);
            prev_move_for_eval = Some(*mv);
            win_eval_result = bit_vidro.win_eval();
        }
        if let EvalValue::Win(v) = win_eval_result.value {
            eval += v;
        }
        for _ in 0..moves.len() {
            bit_vidro.undo_force(moves.pop().unwrap());
        }
    }
    // eval * MAX_EVAL / N as i16
    eval * 10
}

fn main() {
    // let mut bit_vidro = Bitboard::new_initial();
    // println!("result: {}", eval_mon(&mut bit_vidro));
    // return;

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

    let path = "tsumi_log.csv";
    let log_file_obj = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("ログファイルを開けませんでした");
    let log_file = Arc::new(Mutex::new(log_file_obj));
    // CSVのヘッダーを書き込む（プログラム起動時に一度だけ）

    {
        if let Ok(meta) = metadata(path) {
            if meta.len() == 0 {
                writeln!(log_file.lock().unwrap(), " tsumi_found, small_bod")
                    .expect("ヘッダーを書き込めませんでした");
            }
        }
    }

    let tt = Arc::new(Mutex::new(LruCache::new(
        NonZeroUsize::new(100_000).unwrap(),
    )));

    for _ in 0..100 {
        let mut vidro = Bitboard::new_initial();

        let mut move_count = 0;
        let mut prev_move = None;
        const MAX_MOVES: usize = 100;
        const RANDOM_MOVES_UNTIL: usize = 6;

        loop {
            println!("\n--------------------------------");
            println!("{}", vidro.to_string());

            {
                let win_eval_result = vidro.win_eval();
                if win_eval_result.evaluated {
                    match win_eval_result.value {
                        EvalValue::Win(v) => {
                            println!("ゲーム終了 勝者: {}", if v == 1 { "先手" } else { "後手" });
                        }
                        EvalValue::Draw => {
                            println!("ゲーム終了 引き分け");
                        }
                        _ => (),
                    }
                    break;
                }
            }
            if move_count >= MAX_MOVES {
                println!("ゲーム終了 {}手経過により引き分け", MAX_MOVES);
                break;
            }

            let mut best_move: MoveBit;
            if move_count < RANDOM_MOVES_UNTIL {
                println!("----ランダムループを選択----");
                let legal_moves = vidro.generate_legal_move(prev_move);
                if legal_moves.is_empty() {
                    break;
                }
                use rand::seq::SliceRandom;
                best_move = (*legal_moves.choose(&mut rand::thread_rng()).unwrap()).clone();
            } else {
                // let is_turn_humen = vidro.turn == 1;
                let is_turn_humen = false;
                if is_turn_humen {
                    println!("手を選択");
                    let legal_moves = vidro.generate_legal_move(prev_move);
                    MoveBit::print_vec_to_string(&legal_moves);
                    while {
                        best_move = Bitboard::read_to_move();
                        !legal_moves.contains(&best_move)
                    } {}
                } else {
                    println!("思考中...");
                    let search_depth = 5;

                    let log_file_for_thread = Arc::clone(&log_file);
                    let tt_for_thread = Arc::clone(&tt);
                    best_move = match find_best_move(
                        &mut vidro,
                        search_depth,
                        tt_for_thread,
                        log_file_for_thread,
                        prev_move,
                    ) {
                        Some(mv) => mv,
                        None => {
                            println!("指せる手がありません。手番プレイヤーの負けです");
                            break;
                        }
                    };
                }
            }
            println!("\n決定手: {}", best_move.to_string());
            vidro.apply_force(best_move);
            prev_move = Some(best_move);

            move_count += 1;
        }
        println!("\n対局終了");
    }
}
