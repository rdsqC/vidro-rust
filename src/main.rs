mod bitboard;
mod bitboard_console;
mod checkmate_search;
mod eval;
mod eval_value;
use Vec;
use bitboard::{Bitboard, MoveBit};
use bitboard_console::BitboardConsole;
use checkmate_search::{find_mate, find_mate_in_one_move, find_mate_sequence};
use eval::static_evaluation;
use eval_value::{Eval, EvalValue};
use lru::LruCache;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::fs::{File, OpenOptions, metadata};
use std::io::Write;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::{io, usize};

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
    // process: &mut Progress,
    is_root: bool, // ★自分がルートノード（探索の起点）かを知るためのフラグ
    shared_info: Arc<Mutex<SearchInfo>>, // ★情報共有のための構造体
    log_file: &Arc<Mutex<File>>,
    prev_move: Option<MoveBit>,
) -> (i16, Vec<MoveBit>) {
    // process.update(depth, board, tt.len());
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
        let tsumi_result = find_mate_sequence(board, 3, prev_move);

        let static_score = evaluate_for_negamax(board, prev_move);
        //詰み探索を実行
        let tsumi_found = if tsumi_result.is_some() { 1 } else { 0 };

        let log_line = format!("{},{}", tsumi_found, board.to_small_bod());

        //ファイル出力
        // writeln!(log_file.lock().unwrap(), "{}", log_line).expect("ログを書き込めませんでした");

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
            // process,
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
                // info.nodes = process.nodes_searched;
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
                // &mut process,
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
        .expect("ログファイルを開けませんでしたMaintainers and contributors of this project");
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

    loop {
        let mut vidro = Bitboard::new_initial();

        let mut move_count = 0;
        let mut prev_move = None;
        const MAX_MOVES: usize = 100;
        const RANDOM_MOVES_UNTIL: usize = 0;

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
                let is_turn_humen = vidro.turn == 1;
                // let is_turn_humen = false;
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
