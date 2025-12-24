use std::collections::HashSet;

use crate::{
    bitboard::{Bitboard, MoveBit, MoveList},
    eval::{AiModel, GameResult},
    random_state_generator::random_state_generator,
    snapshot::BoardSnapshot,
    snapshot_features::BoardSnapshotFeatures,
};
use rand::{
    Rng,
    distr::{Distribution, weighted::WeightedIndex},
};
use rayon::prelude::*;

pub fn generate_self_play_data(
    random_moves_until: usize,
    current_model: &AiModel,
    past_models: &[AiModel],
    batch_size: usize,
) -> Vec<GameResult> {
    (0..batch_size)
        .into_par_iter()
        .map(|_| {
            let mut rng = rand::rng();

            let p2_model = if !past_models.is_empty() && rng.random_bool(0.3) {
                &past_models[rng.random_range(0..past_models.len())]
            } else {
                current_model
            };

            let (mut board, mut prev_hash) = random_state_generator(random_moves_until);
            let mut history: Vec<BoardSnapshot> = Vec::with_capacity(20);

            let mut seen_state: HashSet<u64> = HashSet::new();

            let mut turn_count = 0;
            while !board.game_over() {
                let current_hash = board.to_compression_bod();

                history.push(board.to_snapshot(prev_hash));

                let state_hash = board.to_compression_bod();
                if seen_state.contains(&state_hash) {
                    break;
                }

                seen_state.insert(state_hash);

                let temp = if turn_count < 3 { 1.5 } else { 0.5 };

                //ターンに合わせてモデルを切り替え
                let model_to_use = if turn_count % 2 == 0 {
                    current_model
                } else {
                    p2_model
                };

                if let Some(mv) = select_move_softmax(&board, model_to_use, temp, prev_hash) {
                    if board
                        .apply_force_with_check_illegal_move(mv, Some(current_hash))
                        .is_err()
                    {
                        panic!("AiModel must not selected illegal move");
                    }
                } else {
                    break;
                }

                prev_hash = Some(current_hash);
                turn_count += 1;
            }

            history.shrink_to_fit();

            let score = match board.win_turn() {
                1 => 1.0,
                -1 => 0.0,
                0 => 0.5,
                _ => {
                    panic!()
                }
            };

            GameResult { history, score }
        })
        .collect()
}

fn select_move_softmax(
    board: &Bitboard,
    ai_model: &AiModel,
    temperature: f32,
    prev_hash: Option<u64>,
) -> Option<MoveBit> {
    let hash = board.to_compression_bod();
    let mut legal_moves = MoveList::new();
    board.generate_legal_moves(&mut legal_moves);

    let scores: Vec<f32> = legal_moves
        .iter()
        .map(|&mv| {
            let mut next_board = board.clone();
            if next_board
                .apply_force_with_check_illegal_move(mv, prev_hash)
                .is_err()
            {
                return f32::NEG_INFINITY;
            }
            let z: f32 = -search(
                &mut next_board,
                ai_model,
                3,
                f32::NEG_INFINITY,
                f32::INFINITY,
                Some(hash),
            );
            z
        })
        .collect();

    let max_score = scores.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

    let weights_prob: Vec<f32> = scores
        .iter()
        .map(|&s| ((s - max_score) / temperature).exp())
        .collect();

    let dist = WeightedIndex::new(&weights_prob).unwrap();
    let mut rng = rand::rng();

    Some(legal_moves[dist.sample(&mut rng)])
}

const MAX_SCORE_ABS: f32 = 30000.0;

fn search(
    board: &mut Bitboard,
    ai_model: &AiModel,
    depth: usize,
    mut alpha: f32,
    beta: f32,
    prev_hash: Option<u64>,
) -> f32 {
    if board.game_over() {
        return MAX_SCORE_ABS * board.win_turn() as f32 * board.turn as f32;
    }
    if depth == 0 {
        let score = ai_model.eval_score(board.to_snapshot(prev_hash).iter_feature_indices());
        return score;
    }

    let mut moves = MoveList::new();
    board.generate_legal_moves(&mut moves);

    if moves.is_empty() {
        return -MAX_SCORE_ABS;
    }

    let mut max_score = f32::NEG_INFINITY;
    for mv in moves {
        if board
            .apply_force_with_check_illegal_move(mv, prev_hash)
            .is_err()
        {
            //-30000と同義
            continue;
        };
        let score = -search(
            board,
            ai_model,
            depth - 1,
            -beta,
            -alpha,
            Some(board.to_compression_bod()),
        );
        board.undo_force(mv);

        if score > max_score {
            max_score = score;
        }

        if max_score >= beta {
            break;
        }
        if max_score > alpha {
            alpha = max_score;
        }
    }

    max_score
}
