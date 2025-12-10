use crate::bitboard::{Bitboard, MoveBit};
use crate::bitboard_console::BitboardConsole;
use crate::checkmate_search::{find_mate, find_mate_in_one_move, find_mate_sequence};
use crate::eval::static_evaluation;
use crate::eval_value::{Eval, EvalValue};
use crate::snapshot::BoardSnapshot;
use Vec;
use lru::LruCache;
use std::fs::{File, OpenOptions, metadata};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const USE_CACHE: bool = true;

const DRAW_SCORE: i16 = 0;
const WIN_LOSE_SCORE: i16 = 30000;

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
pub enum TTFlag {
    Exact,      // このスコアは真の評価値 (alpha < score < beta)
    LowerBound, // このスコアは下限値 (score >= beta, betaカットで得られた)
    UpperBound, // このBitboardは上限値 (score <= alpha, 有望な手が見つからなかった)
}

// 置換表に保存するデータ構造
#[derive(Clone, Copy, Debug)]
pub struct TTEntry {
    score: i16,
    depth: u8, // 保存したときの探索深さ
    flag: TTFlag,
    best_move: MoveBit, // その局面で見つかった最善手
}

pub fn alphabeta<F>(
    board: &mut Bitboard,
    depth: usize,
    mut alpha: i16,
    mut beta: i16,
    tt: &mut LruCache<u64, TTEntry>,
    route: &mut Vec<u64>,
    // process: &mut Progress,
    is_root: bool, // ★自分がルートノード（探索の起点）かを知るためのフラグ
    shared_info: Arc<Mutex<SearchInfo>>, // ★情報共有のための構造体
    prev_move: Option<MoveBit>,
    evaluate: &F,
) -> (i16, Vec<MoveBit>)
where
    F: Fn(&BoardSnapshot) -> i16 + Sync,
{
    // process.update(depth, board, tt.len());
    let mut best_pv = Vec::new();
    // canonical_board(&mut canonical_board_data);
    let hash = board.to_compression_bod();
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
        let tsumi_result = find_mate_sequence(board, 1, prev_move);

        let static_score = evaluate(&board.to_snapshot(prev_move)) * board.turn as i16;
        //詰み探索を実行
        let tsumi_found = if tsumi_result.is_some() { 1 } else { 0 };

        let log_line = format!("{},{}", tsumi_found, board.to_compression_bod());

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
                    TTFlag::Exact => {
                        route.pop();
                        return (entry.score, vec![entry.best_move]);
                    }
                    TTFlag::LowerBound => alpha = alpha.max(entry.score),
                    TTFlag::UpperBound => beta = beta.min(entry.score),
                }
                if alpha >= beta {
                    route.pop();
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
            Some(mv),
            evaluate,
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
        let log_line = format!("{},{}", 1, board.to_compression_bod());
        //ファイル出力
        // writeln!(log_file.lock().unwrap(), "{}", log_line).expect("ログを書き込めませんでした");
    }

    route.pop(); // 探索パスから除去して戻る
    (best_score, best_pv)
}

pub fn mtd_f<F>(
    board: &mut Bitboard,
    f: i16,
    depth: usize,
    tt: Arc<Mutex<LruCache<u64, TTEntry>>>,
    prev_move: Option<MoveBit>,
    evaluate: &F,
) -> (i16, Option<MoveBit>)
where
    F: Fn(&BoardSnapshot) -> i16 + Sync,
{
    let shared_info = Arc::new(Mutex::new(SearchInfo::default()));
    let info_clone_for_ui = shared_info.clone();
    let tt_for_thread = Arc::clone(&tt);
    let mut vidro_for_search = board.clone();

    std::thread::scope(|s| {
        let search_thread = s.spawn(move || {
            let mut prev_socre = f;
            let mut sequence: Vec<MoveBit> = Vec::new();

            for depth_level in 1..=depth {
                let mut tt_guard = tt_for_thread.lock().unwrap();
                let mut g = prev_socre;
                let mut upper_bound = i16::MAX;
                let mut lower_bound = i16::MIN;
                while lower_bound < upper_bound {
                    let beta: i16;
                    if g == lower_bound {
                        beta = g + 1;
                    } else {
                        beta = g;
                    }
                    let mut route = Vec::new();
                    (g, sequence) = alphabeta(
                        &mut vidro_for_search,
                        depth_level,
                        beta - 1,
                        beta,
                        &mut tt_guard,
                        &mut route,
                        true,
                        shared_info.clone(),
                        prev_move,
                        evaluate,
                    );
                    if g < beta {
                        upper_bound = g;
                    } else {
                        lower_bound = g;
                    }
                }
                prev_socre = g;
            }
            (prev_socre, sequence.get(0).map(|mv| *mv))
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
        search_thread.join().unwrap()
    })
}

fn find_best_move<F>(
    board: &mut Bitboard,
    max_depth: usize,
    tt: Arc<Mutex<LruCache<u64, TTEntry>>>,
    prev_move: Option<MoveBit>,
    evaluate: &F,
) -> (i16, Option<MoveBit>)
where
    F: Fn(&BoardSnapshot) -> i16 + Sync,
{
    let shared_info = Arc::new(Mutex::new(SearchInfo::default()));
    let info_clone_for_ui = shared_info.clone();

    let tt_for_thread = Arc::clone(&tt);
    let mut vidro_for_search = board.clone();

    std::thread::scope(|s| {
        let search_thread = s.spawn(move || {
            let mut tt_guard = tt_for_thread.lock().unwrap();

            let mut best_move_overall: Option<MoveBit> = None;
            let mut result_score = 0;

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
                    prev_move,
                    evaluate,
                );
                result_score = score;

                //結果をUIに通知
                {
                    let mut info = shared_info.lock().unwrap();
                    info.score = score;
                    info.depth = depth_run;
                    info.pv = pv_sequence.clone();
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
            (result_score, best_move_overall)
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
    })
}
