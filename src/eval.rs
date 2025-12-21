use rand::{Rng, distr::Uniform};
use serde::{Deserialize, Serialize};

use super::bitboard::{Bitboard, FIELD_BOD, FIELD_BOD_HEIGHT, FIELD_BOD_WIDTH, MoveBit};
use super::checkmate_search::generate_threat_moves;
use super::eval_value::{Eval, EvalValue};

pub fn static_evaluation(vidro: &mut Bitboard, prev_hash: Option<u64>) -> i16 {
    let threats = evaluate_threats(&vidro);
    let have_piece = evaluate_have_piece(&vidro);
    let position = evaluate_position(&vidro);
    let reach = evaluate_reach(vidro, prev_hash);

    // 自分の「詰めろ」になる手の数が多いほど、局面は有利
    let my_threats = generate_threat_moves(vidro, prev_hash).len() as i16;
    let threat_score = my_threats * 25; // 例：1つの脅威手を25点と評価

    // 相手の脅威の数も計算し、評価値から引くとなお良い
    vidro.turn_change();
    let opponent_threats = generate_threat_moves(vidro, prev_hash).len() as i16;
    let opponent_threat_score = opponent_threats * 150;
    vidro.turn_change(); // ★手番を必ず元に戻す

    threats + have_piece * 100 + position + threat_score - opponent_threat_score
}

fn evaluate_position(vidro: &Bitboard) -> i16 {
    let mut score = 0;
    let mut players_bod = vidro.player_bods;
    for p in 0..2 {
        let p_turn = -(p as i16 * 2) + 1;
        while players_bod[p] != 0 {
            let mut idx = players_bod[p].trailing_zeros() as usize;
            idx = idx / 5 + idx % FIELD_BOD_WIDTH as usize;
            score += POSITION_SCORES[idx] * p_turn;
            players_bod[p] &= players_bod[p] - 1;
        }
    }
    score += (vidro.can_set_count_with_turn_idx(0) as i16
        - vidro.can_set_count_with_turn_idx(1) as i16)
        * 10;
    score
}

const POSITION_SCORES: [i16; 25] = [
    10, 0, 9, 0, 10, //
    0, 2, 4, 2, 0, //
    9, 4, 12, 4, 9, //中央 > 角 > 辺の中央 > その他
    0, 2, 4, 2, 0, //
    10, 0, 9, 0, 10, //
];

fn evaluate_have_piece(vidro: &Bitboard) -> i16 {
    vidro.have_piece[1] as i16 - vidro.have_piece[0] as i16
}

fn evaluate_threats(vidro: &Bitboard) -> i16 {
    const OPEN_TWO_SCORE: i16 = 0; // _XX_ (両側が空いている2)
    const SEMI_OPEN_TWO_SCORE: i16 = 300; // OXX_ や _XXO (片側が空いている2)
    const SEMI_OPEN_SPLIT_ONE_SCORE: i16 = 300; //X_X (1つ空きオープン)
    const OPEN_SPLIT_ONE_SCORE: i16 = 0; //上のX_Xに含まれる _X_X_ (1つ空きのオープンな2)

    const MARGIN_WIDTH: u64 = 9;

    let blank: u64 = FIELD_BOD & !(vidro.player_bods[0] | vidro.player_bods[1]); //空白マス

    let mut total_score = 0i16;

    for p in 0..2 {
        let me = vidro.player_bods[p];
        let opp = vidro.player_bods[1 - p];
        let mut player_score = 0i16;

        // 各方向へのシフト量を定義 (7x5盤面用)
        const DIRS: [u64; 4] = [
            1,                // 横
            MARGIN_WIDTH,     // 縦
            MARGIN_WIDTH - 1, // 右上斜め
            MARGIN_WIDTH + 1, // 左上斜め
        ];

        for &d in &DIRS {
            // パターン1: オープンな2 (_XX_)
            // パターン: [空き, 自分, 自分, 空き]
            let pattern_open_two = (blank >> 0) & (me >> d) & (me >> (d * 2)) & (blank >> (d * 3));
            player_score += pattern_open_two.count_ones() as i16 * OPEN_TWO_SCORE;

            // パターン2: 片側が空いた2 (OXX_)
            // パターン: [相手, 自分, 自分, 空き]
            let pattern_semi_open_two_a =
                (opp >> 0) & (me >> d) & (me >> (d * 2)) & (blank >> (d * 3));
            player_score += pattern_semi_open_two_a.count_ones() as i16 * SEMI_OPEN_TWO_SCORE;

            // パターン3: 片側が空いた2 (_XXO)
            // パターン: [空き, 自分, 自分, 相手]
            let pattern_semi_open_two_b =
                (blank >> 0) & (me >> d) & (me >> (d * 2)) & (opp >> (d * 3));
            player_score += pattern_semi_open_two_b.count_ones() as i16 * SEMI_OPEN_TWO_SCORE;

            // パターン4: 1つ空きのオープンな2 (_X_X_)
            // パターン: [空き, 自分, 空き, 自分, 空き]
            let pattern_open_split_one = (blank >> 0)
                & (me >> d)
                & (blank >> (d * 2))
                & (me >> (d * 3))
                & (blank >> (d * 4));
            player_score += pattern_open_split_one.count_ones() as i16 * OPEN_SPLIT_ONE_SCORE;

            let pattern_semi_open_split_one = me & (blank >> d) & (me >> (d * 2));
            player_score +=
                pattern_semi_open_split_one.count_ones() as i16 * SEMI_OPEN_SPLIT_ONE_SCORE;
        }

        // プレイヤー1のスコアは加算、プレイヤー2のスコアは減算
        if p == 0 {
            total_score += player_score;
        } else {
            total_score -= player_score;
        }
    }

    total_score
}

fn evaluate_reach(vidro: &mut Bitboard, prev_hash: Option<u64>) -> i16 {
    vidro.turn_change(); //意図的に手番を書き換え2手差しさせたときに勝利することがあるかを調べる
    let moves = vidro.iter_legal_move();
    let turn = vidro.turn;
    for mv in moves {
        match vidro.apply_force_with_check_illegal_move(mv, prev_hash) {
            Ok(()) => {
                if let EvalValue::Win(value) = vidro.win_eval().value {
                    if value as i8 == turn {
                        vidro.undo_force(mv);
                        vidro.turn_change();
                        return value as i16
                            * (10 - vidro.have_piece[0] - vidro.have_piece[1]) as i16
                            * 15;
                    }
                }
                vidro.undo_force(mv);
            }
            Err(()) => {}
        }
    }
    vidro.turn_change();
    return 0;
}

use crate::snapshot::BoardSnapshot;
use crate::snapshot_features::{BitIter, BoardSnapshotFeatures, NUM_FEATURES};
use rayon::prelude::*;

const LEARNING_RATE: f32 = 1e-3;
const LAMBDA: f32 = 0.05; //正則化係数

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AiModel {
    pub weights: Vec<f32>,
}

#[derive(Debug)]
pub struct GameResult {
    pub history: Vec<BoardSnapshot>,
    pub score: f32, // 1.0: 先手勝ち, 0.0 先手負け
}

impl AiModel {
    pub fn rand_new() -> Self {
        Self {
            weights: (0..NUM_FEATURES)
                .map(|_| rand::random_range(-0.1f32..=0.1f32))
                .collect::<Vec<f32>>(),
        }
    }
    pub fn eval_score(&self, features_iter: impl Iterator<Item = usize>) -> f32 {
        features_iter.map(|n| self.weights[n]).sum()
    }
    pub fn eval_score_from_vec(&self, features: &[usize]) -> f32 {
        features.iter().map(|&n| self.weights[n]).sum()
    }
    pub fn update_from_batch_and_get_update_norm(&mut self, batch: &[GameResult]) -> f32 {
        let batch_size = batch.len() as f32;
        let total_gradients: Vec<f32> = batch
            .par_iter()
            .fold(
                || vec![0.0f32; NUM_FEATURES],
                |mut local_grads, game| {
                    self.accumulate_game_gradient(&mut local_grads, game);
                    local_grads
                },
            )
            .reduce(
                || vec![0.0; NUM_FEATURES],
                |mut a, b| {
                    a.iter_mut()
                        .zip(b.into_iter())
                        .for_each(|(item1, item2)| *item1 += item2);
                    a
                },
            );

        let mut update_norm_square: f32 = 0.0;

        let eta = LEARNING_RATE / batch_size;
        for i in 0..NUM_FEATURES {
            let gradient = total_gradients[i];
            let regularization = LAMBDA * self.weights[i];
            let update_weight = eta * gradient - LEARNING_RATE * regularization;
            self.weights[i] += update_weight; //正則化
            update_norm_square += update_weight.powi(2);
        }

        update_norm_square.sqrt()
    }
    pub fn update_from_batch(&mut self, batch: &[GameResult]) {
        self.update_from_batch_and_get_update_norm(batch);
    }
    fn accumulate_game_gradient(&self, accumulator: &mut Vec<f32>, game: &GameResult) {
        //先手からみた試合の勝敗
        let game_score = game.score;

        for snapshot in game.history.iter() {
            let z: f32 = self.eval_score(snapshot.iter_feature_indices());
            let p = sigmoid(z);

            //turn
            let is_white_turn = snapshot.turn == 1;

            let target = if is_white_turn {
                game_score
            } else {
                1.0 - game_score
            };

            //誤差
            let error = target - p;

            //勾配加算
            snapshot
                .iter_feature_indices()
                .for_each(|idx| accumulator[idx] += error);
        }
    }
    pub fn update_from_snapshot(&mut self, snapshot: BoardSnapshot, target: f32) {
        let z: f32 = self.eval_score(snapshot.iter_feature_indices());
        let p = sigmoid(z);
        //誤差
        let error = target - p;

        //勾配加算
        snapshot.iter_feature_indices().for_each(|idx| {
            let regularization = LAMBDA * self.weights[idx];
            self.weights[idx] += LEARNING_RATE * (error - regularization);
        });
    }
    pub fn weight_norm(&self) -> f32 {
        self.weights.iter().map(|w| w.powi(2)).sum::<f32>().sqrt()
    }
}

pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}
