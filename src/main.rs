mod bitboard;
mod bitboard_console;
mod checkmate_search;
mod eval;
mod eval_value;
mod search;
mod self_match;
mod snapshot;
mod snapshot_features;
use bitboard::{Bitboard, MoveBit};
use bitboard_console::BitboardConsole;
use eval_value::{Eval, EvalValue};
use lru::LruCache;
use rand::seq::IndexedRandom;
use search::mtd_f;
use std::fs::{File, OpenOptions, metadata};
use std::io::Write;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use crate::checkmate_search::generate_threat_moves;
use crate::eval::AiModel;
use crate::self_match::generate_self_play_data;
use crate::snapshot::BoardSnapshot;
use crate::snapshot_features::BoardSnapshotFeatures;

fn main() {
    let mut ai_ctx = AiModel::rand_new();

    let batch_size = 100;
    let epochs = 1000;

    println!(
        "start learning\nepochs:{} batch_size:{}",
        epochs, batch_size
    );

    for epoch in 0..epochs {
        // 自己対局
        let games = generate_self_play_data(&ai_ctx.weights, batch_size);

        //重み更新
        ai_ctx.update_from_batch(&games);

        //ログ出力
        if epoch % 10 == 0 {
            let win_rate = games.iter().map(|g| g.score).sum::<f32>() / batch_size as f32;
            println!("Epoch {}: 先手勝率: {:.3}", epoch, win_rate);
        }
    }

    println!("学習完了");

    let evaluate = |snapshot: &BoardSnapshot| {
        let z = ai_ctx.eval_score(snapshot.iter_feature_indices());
        ((z * 100.0) as i16).clamp(-29000, 29000)
    };

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
                best_move = (*legal_moves.choose(&mut rand::rng()).unwrap()).clone();
            } else {
                let is_turn_humen = vidro.turn == 1;
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

                    let tt_for_thread = Arc::clone(&tt);

                    // tt.lock().unwrap().clear();

                    let score;
                    (score, best_move) = {
                        let result = mtd_f(
                            &mut vidro,
                            0,
                            search_depth,
                            tt_for_thread,
                            prev_move,
                            &evaluate,
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
