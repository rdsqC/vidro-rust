mod bitboard;
mod bitboard_console;
mod checkmate_search;
mod eval;
mod eval_value;
mod search;
use Vec;
use bitboard::{Bitboard, MoveBit};
use bitboard_console::BitboardConsole;
use checkmate_search::{find_mate, find_mate_in_one_move, find_mate_sequence};
use eval::static_evaluation;
use eval_value::{Eval, EvalValue};
use lru::LruCache;
use rand::seq::SliceRandom;
use rand::thread_rng;
use search::mtd_f;
use std::fs::{File, OpenOptions, metadata};
use std::io::Write;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::{io, usize};

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
                    let search_depth = 9;

                    // let log_file_for_thread = Arc::clone(&log_file);
                    // let tt_for_thread = Arc::clone(&tt);
                    //
                    // tt.lock().unwrap().clear();
                    //
                    // let score;
                    // (score, best_move) = {
                    //     let result = find_best_move(
                    //         &mut vidro,
                    //         search_depth,
                    //         tt_for_thread,
                    //         log_file_for_thread,
                    //         prev_move,
                    //     );
                    //     if result.1.is_some() {
                    //         (result.0, result.1.unwrap())
                    //     } else {
                    //         println!("指せる手がありません。手番プレイヤーの負けです");
                    //         break;
                    //     }
                    // };
                    // println!(
                    //     "\nalphabeta 決定手: {} 評価値{}",
                    //     best_move.to_string(),
                    //     score
                    // );

                    let log_file_for_thread = Arc::clone(&log_file);
                    let tt_for_thread = Arc::clone(&tt);

                    // tt.lock().unwrap().clear();

                    let score;
                    (score, best_move) = {
                        let result = mtd_f(
                            &mut vidro,
                            0,
                            search_depth,
                            tt_for_thread,
                            log_file_for_thread,
                            prev_move,
                        );
                        if result.1.is_some() {
                            (result.0, result.1.unwrap())
                        } else {
                            println!("指せる手がありません。手番プレイヤーの負けです");
                            break;
                        }
                    };
                    println!("\nmtd-f 決定手: {} 評価値{}", best_move.to_string(), score);
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
