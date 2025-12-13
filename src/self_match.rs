use core::f32;
use std::collections::HashSet;

use crate::{
    bitboard::{Bitboard, MoveBit},
    bitboard_console::print_u64,
    eval::{AiModel, GameResult},
    eval_value,
    random_state_generator::random_state_generator,
    snapshot::BoardSnapshot,
    snapshot_features::BoardSnapshotFeatures,
};
use rand::distr::{Distribution, weighted::WeightedIndex};
use rayon::prelude::*;

pub fn generate_self_play_data(
    random_moves_until: usize,
    ai_model: &AiModel,
    batch_size: usize,
) -> Vec<GameResult> {
    (0..batch_size)
        .into_par_iter()
        .map(|_| {
            let (mut board, mut prev_move) = random_state_generator(random_moves_until);
            let mut history: Vec<BoardSnapshot> = Vec::with_capacity(20);

            let mut seen_state: HashSet<u64> = HashSet::new();

            let mut turn_count = 0;
            while !board.game_over() {
                history.push(board.to_snapshot(prev_move));

                let state_hash = board.to_compression_bod();
                if seen_state.contains(&state_hash) {
                    break;
                }

                seen_state.insert(state_hash);

                let temp = if turn_count < 5 { 1.5 } else { 0.5 };
                if let Some(mv) = select_move_softmax(&board, ai_model, temp, prev_move) {
                    board.apply_force(mv);
                    prev_move = Some(mv);
                } else {
                    break;
                }

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
    prev_move: Option<MoveBit>,
) -> Option<MoveBit> {
    let legal_moves = board.generate_legal_move(prev_move);
    if legal_moves.is_empty() {
        return None;
    }

    let scores: Vec<f32> = legal_moves
        .iter()
        .map(|&mv| {
            let mut next_board = board.clone();
            next_board.apply_force(mv);
            let z: f32 = -search(
                &mut next_board,
                ai_model,
                3,
                f32::NEG_INFINITY,
                f32::INFINITY,
                prev_move,
            ); //常に先手の勝率を予測しているため反転させる
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

fn search(
    board: &mut Bitboard,
    ai_model: &AiModel,
    depth: usize,
    mut alpha: f32,
    beta: f32,
    prev_move: Option<MoveBit>,
) -> f32 {
    if board.game_over() {
        return 30000.0 * board.win_turn() as f32 * board.turn as f32;
    }
    if depth == 0 {
        let score = ai_model.eval_score(board.to_snapshot(prev_move).iter_feature_indices())
            * board.turn as f32;
        return score;
    }

    let moves = board.generate_legal_move(prev_move);
    if moves.is_empty() {
        return ai_model.eval_score(board.to_snapshot(prev_move).iter_feature_indices())
            * board.turn as f32;
    }

    let mut max_score = f32::NEG_INFINITY;
    for mv in moves {
        board.apply_force(mv);
        let score = -search(board, ai_model, depth - 1, -beta, -alpha, Some(mv));
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
