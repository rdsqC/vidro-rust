use std::collections::HashSet;

use crate::{
    bitboard::{Bitboard, MoveBit},
    bitboard_console::print_u64,
    eval::GameResult,
    random_state_generator::random_state_generator,
    snapshot::BoardSnapshot,
    snapshot_features::BoardSnapshotFeatures,
};
use rand::distr::{Distribution, weighted::WeightedIndex};
use rayon::prelude::*;

pub fn generate_self_play_data(
    random_moves_until: usize,
    current_weights: &[f32],
    batch_size: usize,
) -> Vec<GameResult> {
    (0..batch_size)
        .into_par_iter()
        .map(|_| {
            let (mut board, mut prev_move) = random_state_generator(random_moves_until);
            let mut history: Vec<BoardSnapshot> = Vec::with_capacity(100);

            let mut seen_state: HashSet<u64> = HashSet::new();

            let mut turn_count = 0;
            while !board.game_over() {
                history.push(board.to_snapshot(prev_move));

                let state_hash = board.to_compression_bod();
                if seen_state.contains(&state_hash) {
                    break;
                }

                seen_state.insert(state_hash);

                let temp = if turn_count < 20 { 1.5 } else { 0.5 };
                if let Some(mv) = select_move_softmax(&board, current_weights, temp, prev_move) {
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
    weights: &[f32],
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

            // if (next_board.player_bods[0] & next_board.player_bods[1]) != 0 {
            //     println!("CRITICAL ERROR: Bitboard overlap detected!");
            //     println!("Move: {:?}", mv);
            //     print_u64("P1", next_board.player_bods[0]);
            //     print_u64("P2", next_board.player_bods[1]);
            //     print_u64(
            //         "Overlap",
            //         next_board.player_bods[0] & next_board.player_bods[1],
            //     );
            //     panic!("Data corruption in apply_force");
            // }

            let snapshot = next_board.to_snapshot(Some(mv));
            let z: f32 = snapshot
                .iter_feature_indices()
                .map(|idx| weights[idx])
                .sum();

            z * snapshot.turn as f32 //常に先手の勝率を予測しているため反転させる
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
