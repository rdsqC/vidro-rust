use crate::{
    bitboard::{Bitboard, MoveBit},
    eval::GameResult,
    snapshot::BoardSnapshot,
    snapshot_features::BoardSnapshotFeatures,
};
use rand::distr::{Distribution, weighted::WeightedIndex};
use rayon::prelude::*;

pub fn generate_self_play_data(current_weights: &[f32], batch_size: usize) -> Vec<GameResult> {
    (0..batch_size)
        .into_par_iter()
        .map(|_| {
            let mut board = Bitboard::new_initial();
            let mut history: Vec<BoardSnapshot> = Vec::new();
            let mut prev_move: Option<MoveBit> = None;

            let mut turn_count = 0;
            while board.game_over() {
                history.push(board.to_snapshot(prev_move));

                let temp = if turn_count < 20 { 1.5 } else { 0.5 };

                if let Some(mv) = select_move_softmax(&board, current_weights, temp, prev_move) {
                    board.apply_force(mv);
                    prev_move = Some(mv);
                } else {
                    break;
                }

                turn_count += 1;
            }

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
