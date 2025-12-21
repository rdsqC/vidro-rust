mod bitboard;
mod bitboard_console;
mod checkmate_search;
mod eval;
mod eval_value;
mod pre_train;
mod random_state_generator;
mod search;
mod self_match;
mod snapshot;
mod util;

mod snapshot_features;
use bitboard::{Bitboard, MoveBit};
use bitboard_console::{BitboardConsole, print_u64};
use eval_value::{Eval, EvalValue};

use clap::{Parser, Subcommand};
use lru::LruCache;
use pre_train::pre_train_with_manual_eval;
use rand::seq::IndexedRandom;
use search::mtd_f;
use std::fs::{File, OpenOptions, metadata};
use std::io::Write;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use crate::bitboard::MoveList;
use crate::eval::{AiModel, sigmoid};
use crate::self_match::generate_self_play_data;
use crate::snapshot::BoardSnapshot;
use crate::snapshot_features::{BoardSnapshotFeatures, FEATURE_LINES, NUM_FEATURES};
use crate::util::{load_model, save_model};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Train {
        #[arg(short, long, default_value_t = 10000)]
        epochs: usize,

        #[arg(short, long, default_value_t = 320)]
        batch_size: usize,
    },
    Play {
        #[arg(short, long, default_value_t = 5)]
        depth: usize,

        //先手1 後手0
        #[arg(short, long, default_value_t = 1)]
        human_turn: i8,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        &Commands::Train { epochs, batch_size } => {
            train_mode(epochs, batch_size);
        }
        &Commands::Play { depth, human_turn } => {
            play_mode(depth, human_turn);
        }
    }
}

fn train_mode(epochs: usize, batch_size: usize) {
    const RANDOM_MOVES_UNTIL: usize = 5;

    println!("NUM_FEATURES: {}", NUM_FEATURES);

    let mut ai_ctx;

    let load_path = "model_latest.bin";
    if std::path::Path::new(load_path).exists() {
        match load_model(load_path) {
            Ok(m) => ai_ctx = m,
            Err(e) => {
                eprintln!("Load failure: {}\n(Create new)", e);
                ai_ctx = AiModel::rand_new();
                pre_train_with_manual_eval(&mut ai_ctx, 1000000, 15);
            }
        }
    } else {
        ai_ctx = AiModel::rand_new();
        pre_train_with_manual_eval(&mut ai_ctx, 1000000, 15);
    }

    // let batch_size = 320;
    // let epochs = 10000;

    let mut past_models_pool: Vec<AiModel> = Vec::new();

    println!(
        "start learning\nepochs:{} batch_size:{}",
        epochs, batch_size
    );

    println!("start self match");
    for epoch in 1..=epochs {
        let opponent_pool: &Vec<AiModel> = if past_models_pool.len() > 5 && epoch % 2 == 0 {
            &past_models_pool
        } else {
            &vec![]
        };

        // 自己対局
        let games = generate_self_play_data(RANDOM_MOVES_UNTIL, &ai_ctx, opponent_pool, batch_size);

        let weight_norm = ai_ctx.weight_norm();

        //重み更新
        let update_norm = ai_ctx.update_from_batch_and_get_update_norm(&games);
        let max_weight = ai_ctx
            .weights
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        println!(
            "Update Ratio: {:.3e}, MaxWeight: {}",
            update_norm / weight_norm,
            max_weight
        );

        //最新データの保存
        if let Err(e) = save_model(&ai_ctx, "model_latest.bin") {
            eprintln!("Err to save: {}", e);
        };

        if epoch % 100 == 0 {
            let backup_path = format!("models/model_epoch_{}.bin", epoch);
            std::fs::create_dir_all("models").unwrap();
            if let Err(e) = save_model(&ai_ctx, &backup_path) {
                eprintln!("Failed to save backup\n{}", e);
            }
        }

        //ログ出力
        if epoch % 10 == 0 {
            let win_rate = games.iter().map(|g| g.score).sum::<f32>() / batch_size as f32;
            let ave_moves =
                games.iter().map(|g| g.history.len() as f32).sum::<f32>() / games.len() as f32;
            println!(
                "Epoch {}: WhiteWinRate: {:.3} AveMoves:{:.3}",
                epoch, win_rate, ave_moves
            );
        }

        if epoch % 20 == 0 {
            past_models_pool.push(ai_ctx.clone());
            if past_models_pool.len() > 100 {
                //100を超えたら古いモデルデータを削除
                past_models_pool.remove(0);
            }
        }
    }

    println!("学習完了");
}

const EVAL_VALUE_MALTIPLIER: f32 = 100.0;

fn play_mode(depth: usize, human_turn: i8) {
    let load_path = "model_latest.bin";

    if !std::path::Path::new(load_path).exists() {
        println!("load_path:{} i not exists. ", load_path);
        return;
    }

    let Ok(ai_ctx) = load_model(load_path) else {
        println!("Failed load model file");
        return;
    };

    let evaluate = |snapshot: &BoardSnapshot| {
        let z = ai_ctx.eval_score(snapshot.iter_feature_indices());
        ((z * EVAL_VALUE_MALTIPLIER) as i16).clamp(-29000, 29000)
    };

    let tt = Arc::new(Mutex::new(LruCache::new(
        NonZeroUsize::new(100_000).unwrap(),
    )));

    loop {
        let mut vidro = Bitboard::new_initial();

        let mut move_count = 0;
        let mut prev_hash: Option<u64> = None;
        const MAX_MOVES: usize = 100;
        const RANDOM_MOVES_UNTIL: usize = 0;

        loop {
            println!("\n--------------------------------");
            println!("{}", vidro.to_string());

            let relative = vidro.to_snapshot(prev_hash).to_relative();
            // print_u64("relative white", relative.p1);
            // print_u64("relative black", relative.p2);
            println!(
                "white have: {}\nblack have: {}",
                relative.p1_hand_piece, relative.p2_hand_piece
            );

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
                let mut moves = MoveList::new();
                vidro.generate_legal_moves(&mut moves);
                if moves.is_empty() {
                    break;
                }
                best_move = (*moves.choose(&mut rand::rng()).unwrap()).clone();
            } else {
                let is_turn_humen = vidro.turn == human_turn * 2 - 1;
                if is_turn_humen {
                    println!("手を選択");
                    let legal_moves = vidro
                        .iter_legal_move()
                        .into_iter()
                        .filter(move |&mv| !vidro.check_illegal_move(mv, prev_hash))
                        .collect();
                    MoveBit::print_vec_to_string(&legal_moves);
                    while {
                        best_move = Bitboard::read_to_move();
                        !legal_moves.contains(&best_move)
                    } {}
                } else {
                    println!("思考中...");

                    let tt_for_thread = Arc::clone(&tt);

                    // tt.lock().unwrap().clear();

                    let score;
                    (score, best_move) = {
                        let result =
                            mtd_f(&mut vidro, 0, depth, tt_for_thread, prev_hash, &evaluate);
                        if result.1.is_some() {
                            (result.0, result.1.unwrap())
                        } else {
                            println!("指せる手がありません。手番プレイヤーの負けです");
                            break;
                        }
                    };
                    println!(
                        "\nmtd-f 決定手: {} 評価値{} 勝率: {}",
                        best_move.to_string(),
                        score,
                        sigmoid(score as f32 / EVAL_VALUE_MALTIPLIER)
                    );
                }
            }
            println!("\n決定手: {}", best_move.to_string());
            prev_hash = Some(vidro.to_compression_bod());
            match vidro.apply_force_with_check_illegal_move(best_move, prev_hash) {
                Ok(()) => {}
                Err(()) => {
                    println!("反則手。手番プレイヤーの負けです");
                    break;
                }
            };

            move_count += 1;
        }
        println!("\n対局終了");
    }
}
