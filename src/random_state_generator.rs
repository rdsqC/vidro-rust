use rand::seq::IndexedRandom;

use crate::bitboard::{Bitboard, MoveBit};

pub fn random_state_generator(num_turn: usize) -> (Bitboard, Option<MoveBit>) {
    let mut board = Bitboard::new_initial();
    let mut prev_move: Option<MoveBit> = None;
    for _ in 0..num_turn {
        let legal_moves = board.generate_legal_move(prev_move);
        if legal_moves.is_empty() {
            break;
        }
        let choosed_move = (*legal_moves.choose(&mut rand::rng()).unwrap()).clone();
        board.apply_force(choosed_move);
        prev_move = Some(choosed_move);
    }

    return (board, prev_move);
}
