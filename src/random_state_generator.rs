use rand::seq::IndexedRandom;

use crate::bitboard::{Bitboard, MoveBit};

pub fn random_state_generator(num_turn: usize) -> (Bitboard, Option<u64>) {
    let mut board = Bitboard::new_initial();
    let mut prev_hash: Option<u64> = None;
    for _ in 0..num_turn {
        let hash = board.to_compression_bod();
        let legal_moves = board.generate_legal_move();
        if legal_moves.is_empty() {
            break;
        }

        let mut choosed_move;
        while {
            choosed_move = (*legal_moves.choose(&mut rand::rng()).unwrap()).clone();
            board
                .apply_force_with_check_illegal_move(choosed_move, prev_hash)
                .is_ok()
        } {}
        prev_hash = Some(hash);
    }

    return (board, prev_hash);
}
