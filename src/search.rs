use crate::bitboard::{self, Bitboard, MoveBit, MoveList};
use crate::checkmate_search::{checkmate_in_one_move, find_mate_sequence};
use crate::eval::{sigmoid, static_evaluation};
use crate::search;
use crate::snapshot::BoardSnapshot;
use Vec;
use arrayvec::ArrayVec;
use lru::LruCache;
use rayon::collections::hash_map;
use std::collections::{HashMap, HashSet};
use std::ptr::null;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{i16, i32, thread};

const USE_CACHE: bool = true;

const DRAW_SCORE: i16 = 0;
const WIN_LOSE_SCORE: i16 = 30000;

pub const EVAL_VALUE_MALTIPLIER: f32 = 100.0;

fn evaluate_for_negamax(board: &mut Bitboard, prev_hash: Option<u64>) -> i16 {
    // eval_mon(board, prev_move)
    static_evaluation(board, prev_hash) * board.turn as i16
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

const MIN_MATE_SCORE: i16 = WIN_LOSE_SCORE - 1000;

fn score_to_tt(score: i16, ply: usize) -> i16 {
    if score > MIN_MATE_SCORE {
        score + ply as i16
    } else if score < -MIN_MATE_SCORE {
        score - ply as i16
    } else {
        score
    }
}

fn score_from_tt(score: i16, ply: usize) -> i16 {
    if score > MIN_MATE_SCORE {
        score - ply as i16
    } else if score < -MIN_MATE_SCORE {
        score + ply as i16
    } else {
        score
    }
}

const STATIC_EVAL_SORTING_DEPTH: usize = 2;

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
    prev_hash: Option<u64>,
    evaluate: &F,
    ply: usize,
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
    if board.game_over() {
        route.pop();

        let win_sign = board.win_turn() * board.turn as i16;
        let abs_socre = WIN_LOSE_SCORE - ply as i16;

        let score = win_sign * abs_socre;
        return (score, Vec::new());
    }

    if depth == 0 {
        route.pop();
        let tsumi_result = find_mate_sequence(board, 1, prev_hash);

        let static_score = evaluate(&board.to_snapshot(prev_hash));
        //詰み探索を実行
        let tsumi_found = if tsumi_result.is_some() { 1 } else { 0 };

        let log_line = format!("{},{}", tsumi_found, board.to_compression_bod());

        //ファイル出力
        // writeln!(log_file.lock().unwrap(), "{}", log_line).expect("ログを書き込めませんでした");

        if let Some(mate_sequence) = tsumi_result {
            // board.print_data();
            // 詰みを発見！スコアを「勝ち」に格上げし、手順も返す
            let mate_score = WIN_LOSE_SCORE - (mate_sequence.len() + ply) as i16;
            return (mate_score, mate_sequence);
        }
        return (static_score, best_pv);
    }

    let original_alpha = alpha;
    let original_beta = beta;
    let mut best_move_from_tt: Option<MoveBit> = None;
    //置換表参照
    if USE_CACHE {
        if let Some(entry) = tt.get(&hash) {
            if entry.depth as usize >= depth {
                let tt_score = score_from_tt(entry.score, ply);

                match entry.flag {
                    TTFlag::Exact => {
                        route.pop();
                        return (tt_score, vec![entry.best_move]);
                    }
                    TTFlag::LowerBound => alpha = alpha.max(tt_score),
                    TTFlag::UpperBound => beta = beta.min(tt_score),
                }
                if alpha >= beta {
                    route.pop();
                    return (tt_score, vec![entry.best_move]);
                }
            }
            best_move_from_tt = Some(entry.best_move);
        }
    }

    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    let is_sort = is_root || depth >= STATIC_EVAL_SORTING_DEPTH;

    if is_sort {
        moves.sort_by_cached_key(|&mv| {
            let move_score: i16;
            if Some(mv) == best_move_from_tt {
                move_score = i16::MAX;
            } else {
                match board.apply_force_with_check_illegal_move(mv, prev_hash) {
                    Ok(()) => {
                        move_score = -evaluate(&board.to_snapshot(Some(hash)));
                        board.undo_force(mv);
                    }
                    Err(()) => {
                        move_score = i16::MIN;
                    }
                }
            }

            //降順にするため反転
            -move_score
        });
    } else {
        if let Some(tt_move) = best_move_from_tt {
            if let Some(pos) = moves.iter().position(|m| *m == tt_move) {
                let m = moves.remove(pos);
                moves.insert(0, m);
            }
        }
    }

    let mut best_score = i16::MIN;
    let mut best_move: Option<MoveBit> = None;

    for (i, &mv) in moves.iter().enumerate() {
        //手を実行
        if board
            .apply_force_with_check_illegal_move(mv, prev_hash)
            .is_err()
        {
            continue;
        }

        let score;
        let can_lmr = depth >= 3 && i >= 4 && !is_root && !checkmate_in_one_move(board, Some(hash));

        let mut child_pv;
        if i == 0 || !is_sort {
            //その手ができた場合
            let (s, pv) = alphabeta(
                board,
                depth - 1,
                -beta,
                -alpha,
                tt,
                route,
                // process,
                false,
                shared_info.clone(),
                Some(hash),
                evaluate,
                ply + 1,
            );
            score = -s;
            child_pv = pv;
        } else {
            let mut reduction = 0;
            if can_lmr {
                reduction = 1;
                if i >= 10 {
                    reduction = 2;
                }

                if i >= 25 {
                    reduction = 3;
                }

                //残り深さが0にならないようにする
                if depth <= 1 + reduction {
                    reduction = depth - 2;
                }
            }

            let (s, _) = alphabeta(
                board,
                depth - 1 - reduction,
                -alpha - 1,
                -alpha,
                tt,
                route,
                false,
                shared_info.clone(),
                Some(hash),
                evaluate,
                ply + 1,
            );
            let mut temp_score = -s;

            if temp_score > alpha && reduction > 0 {
                let (s, _) = alphabeta(
                    board,
                    depth - 1,
                    -alpha - 1,
                    -alpha,
                    tt,
                    route,
                    false,
                    shared_info.clone(),
                    Some(hash),
                    evaluate,
                    ply + 1,
                );
                temp_score = -s;
            }

            if temp_score > alpha && temp_score < beta {
                let (s, pv) = alphabeta(
                    board,
                    depth - 1,
                    -beta,
                    -alpha,
                    tt,
                    route,
                    // process,
                    false,
                    shared_info.clone(),
                    Some(hash),
                    evaluate,
                    ply + 1,
                );
                score = -s;
                child_pv = pv;
            } else {
                score = temp_score;
                child_pv = Vec::new();
            }
        }
        board.undo_force(mv); //Bitboardに戻す
        if best_score < score {
            best_score = score;
            best_move = Some(mv);
            best_pv.clear();
            best_pv.push(mv);
            best_pv.append(&mut child_pv);
            if is_root {
                let mut info = shared_info.lock().unwrap();
                info.score = best_score;
                info.pv = best_pv.clone();
                info.depth = depth;
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
            } else if best_score >= original_beta {
                TTFlag::LowerBound
            } else {
                TTFlag::Exact
            };

            let tt_socre_to_save = score_to_tt(best_score, ply);

            let new_entry = TTEntry {
                best_move: mv,
                score: tt_socre_to_save,
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
    prev_hash: Option<u64>,
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
                        prev_hash,
                        evaluate,
                        0,
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
                    "\rDepth: {:2}, Score: {:6}, WinRate: {:.3}, Nodes: {:8}, PV: {:<50}",
                    info.depth,
                    info.score,
                    sigmoid(info.score as f32 / EVAL_VALUE_MALTIPLIER),
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

pub fn find_best_move<F>(
    board: &mut Bitboard,
    depth: usize,
    tt: Arc<Mutex<LruCache<u64, TTEntry>>>,
    prev_hash: Option<u64>,
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
        let builder = std::thread::Builder::new()
            .name("search_thread".into())
            .stack_size(32 * 1024 * 1024);
        let search_thread = builder
            .spawn_scoped(s, move || {
                let mut tt_guard = tt_for_thread.lock().unwrap();

                let mut best_move_overall: Option<MoveBit> = None;
                let mut result_score = 0;

                //反復深化ループ
                for depth_run in 0..=depth {
                    let mut route = Vec::new();

                    //ルートノードで探索
                    let (score, pv_sequence) = alphabeta(
                        &mut vidro_for_search,
                        depth_run,
                        i16::MIN + depth as i16,
                        i16::MAX - depth as i16,
                        &mut tt_guard,
                        &mut route,
                        // &mut process,
                        true,
                        shared_info.clone(),
                        prev_hash,
                        evaluate,
                        0,
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
            })
            .expect("faild start-up search_thread");

        println!("探索開始...");
        loop {
            thread::sleep(Duration::from_millis(200));
            {
                let info = info_clone_for_ui.lock().unwrap();
                print!(
                    "\rDepth: {:2}, Score: {:6}, WinRate: {:.3}, Nodes: {:8}, PV: {:<50}",
                    info.depth,
                    info.score,
                    sigmoid(info.score as f32 / EVAL_VALUE_MALTIPLIER),
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

#[derive(Debug, Clone, Copy)]
pub enum MateValue {
    Unknown,
    Mate(i8),
}

pub fn find_mate(
    board: &mut Bitboard,
    depth: usize,
    route: &mut HashSet<u64>,
    prev_hash: Option<u64>,
    mut alpha: i8,
    beta: i8,
) -> MateValue {
    let hash = board.to_compression_bod();

    if route.contains(&hash) {
        return MateValue::Mate(0);
    } else {
    }

    let eval = board.win_turn() as i8;
    if eval != 0 {
        return MateValue::Mate(eval * board.turn);
    }

    if depth == 0 {
        return MateValue::Unknown;
    }

    //子ノードを作ることが確定したらrouteに追加
    route.insert(hash);

    let mut moves: MoveList = MoveList::new();
    board.generate_legal_moves(&mut moves);

    let mut max_score = -1i8;
    let mut is_contains_unknown = false;
    for mv in moves {
        if !board
            .apply_force_with_check_illegal_move(mv, prev_hash)
            .is_err()
        {
            let eval = find_mate(board, depth - 1, route, Some(hash), -beta, -alpha);
            board.undo_force(mv);

            if let MateValue::Mate(s) = eval {
                let score = -s;

                //最高点数の手を見つけた場合即座に終了
                if score == 1 {
                    route.remove(&hash);
                    return MateValue::Mate(1);
                }

                if beta <= score {
                    route.remove(&hash);
                    return MateValue::Mate(score);
                }

                if max_score < score {
                    max_score = score;
                    if alpha < score {
                        alpha = score;
                    }
                }
            } else {
                is_contains_unknown = true;
            }
        }
    }

    route.remove(&hash);
    if max_score == 1 {
        MateValue::Mate(1)
    } else if is_contains_unknown {
        MateValue::Unknown
    } else {
        MateValue::Mate(max_score)
    }
}
