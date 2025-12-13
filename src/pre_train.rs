use crate::{
    bitboard::MoveBit,
    eval::{AiModel, sigmoid, static_evaluation},
    random_state_generator::random_state_generator,
};

pub fn pre_train_with_manual_eval(ai_model: &mut AiModel, num_train: usize, num_turn: usize) {
    println!("start pre train");

    for _ in 0..num_train {
        let (mut random_state, prev_move) = random_state_generator(num_turn);
        let target: f32 = sigmoid(static_evaluation(&mut random_state, prev_move) as f32);

        let snapshot = random_state.to_snapshot(prev_move);
        ai_model.update_from_snapshot(snapshot, target);
    }
}
