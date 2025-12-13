use super::bitboard::{Bitboard, MoveBit};
use super::eval_value::EvalValue;
use Vec;
use std::usize;

pub fn find_mate_in_one_move(vidro: &mut Bitboard, prev_hash: Option<u64>) -> Option<MoveBit> {
    let moves = vidro.generate_legal_move(); //詰み探索には千日手を除くことはしなくてよい。積んでいる局面になったときに千日手盤面になることはないため
    let turn = vidro.turn;
    for &mv in &moves {
        if vidro
            .apply_force_with_check_illegal_move(mv, prev_hash)
            .is_err()
        {
            continue;
        };
        if let EvalValue::Win(value) = vidro.win_eval().value {
            if value as i8 == turn {
                vidro.undo_force(mv);
                return Some(mv);
            }
        }
        vidro.undo_force(mv);
    }
    None
}

//先後最善を指した時の詰み手順
pub fn find_mate_sequence(
    vidro: &mut Bitboard,
    max_depth: usize,
    prev_hash: Option<u64>,
) -> Option<Vec<MoveBit>> {
    if vidro.have_piece[((-vidro.turn + 1) / 2) as usize] > 2 {
        return None;
    }

    let mut sequence = Vec::new();
    //詰みがあるかどうかをしらべてある場合は手順を構築する
    let result =
        find_mate_sequence_recursive(vidro, max_depth, usize::MIN, usize::MAX, true, prev_hash);

    if let Some((_, first_move)) = result {
        //手順構築
        sequence.push(first_move);

        if vidro
            .apply_force_with_check_illegal_move(first_move, prev_hash)
            .is_err()
        {
            panic!("move to apply must not be illegal move");
        };

        let mut idx = 0;
        while !vidro.win_eval().evaluated {
            if sequence.len() >= max_depth {
                break;
            }

            let remaining_depth = (max_depth - sequence.len()) as usize;

            //受け手と攻め手を入れ替え
            let is_attacker = sequence.len() % 2 == 0;
            if let Some((_, best_next_move)) = find_mate_sequence_recursive(
                vidro,
                remaining_depth,
                usize::MIN,
                usize::MAX,
                is_attacker,
                prev_hash,
            ) {
                if vidro
                    .apply_force_with_check_illegal_move(best_next_move, prev_hash)
                    .is_err()
                {
                    panic!("move to apply must not be illegal move");
                };
                sequence.push(best_next_move);
            } else {
                //手順が見つからなかった(バグの可能性が高い)
                for &mv in sequence.iter().rev() {
                    vidro.undo_force(mv);
                }
                return None;
            }
            idx += 1;
        }

        //見つかった手順を使って盤面を呼び出し前の状態に復元
        for &mv in sequence.iter().rev() {
            vidro.undo_force(mv);
        }

        Some(sequence)
    } else {
        None
    }
}

fn find_mate_sequence_recursive(
    vidro: &mut Bitboard,
    depth: usize,
    alpha: usize,
    beta: usize,
    is_attacker: bool,
    prev_hash: Option<u64>,
) -> Option<(usize, MoveBit)> {
    let hash = vidro.to_compression_bod();

    //一手詰め判定
    if is_attacker {
        if let Some(mv) = find_mate_in_one_move(vidro, prev_hash) {
            return Some((depth, mv));
        }
    }

    if depth == 0 {
        return None;
    }

    let mut alpha = alpha;
    let mut beta = beta;

    if is_attacker {
        //見つかったときのdepthが大きい物(短く詰ませる)手を探す
        let attacking_moves = vidro.generate_legal_move();
        if attacking_moves.is_empty() {
            return None;
        }

        let mut max_depth_found = usize::MIN; //最終的な詰みの深さ
        let mut best_move: Option<MoveBit> = None;

        for mv in attacking_moves {
            if vidro
                .apply_force_with_check_illegal_move(mv, prev_hash)
                .is_err()
            {
                continue;
            }
            let result =
                find_mate_sequence_recursive(vidro, depth - 1, alpha, beta, false, Some(hash));
            vidro.undo_force(mv); //ミスを防ぐためにすぐ戻す
            //
            // 相手の手番で再帰呼び出し
            if let Some((found_depth, _)) = result {
                if max_depth_found < found_depth {
                    //最善が更新された
                    max_depth_found = found_depth;
                    best_move = Some(mv.clone());
                }
                alpha = alpha.max(max_depth_found);
                if alpha >= beta {
                    break;
                }
            }
        }

        best_move.map(|mv| (max_depth_found, mv))
    } else {
        //見つかったときのdepthが小さい物(長く詰まされる)手を探す
        //特に効率の良い守る手を見つける方法はないため合法手から絞り込むことにする
        let defending_moves = vidro.generate_legal_move();
        //合法手が一つもないということは起きないため空の場合は考えない
        // if defending_moves.is_empty() {
        // return Some(depth + 1);
        // }

        let mut min_depth_found = usize::MAX;
        let mut best_move: Option<MoveBit> = None;

        //相手の手番で再帰呼び出し
        for mv in defending_moves {
            if vidro
                .apply_force_with_check_illegal_move(mv, prev_hash)
                .is_err()
            {
                continue;
            }
            let result =
                find_mate_sequence_recursive(vidro, depth - 1, alpha, beta, true, Some(hash));
            vidro.undo_force(mv);

            if result.is_none() {
                return None;
            }

            if let Some((found_depth, _)) = result {
                //詰む場合
                if found_depth < min_depth_found {
                    //最善が更新された
                    min_depth_found = found_depth;
                    best_move = Some(mv.clone());
                }
                beta = beta.min(min_depth_found);
                if beta <= alpha {
                    break;
                }
            }
        }

        //ここで詰まない場合が一つもない
        //すなわち必ず詰むのでvalueを返却
        best_move.map(|mv| (min_depth_found, mv))
    }
}

// main関数などから呼び出すためのラッパー関数
pub fn find_mate(
    vidro: &mut Bitboard,
    max_depth: usize,
    prev_hash: Option<u64>,
) -> Option<MoveBit> {
    let mut mate_move = MoveBit::from_idx(0, 8);
    if find_mate_recursive(vidro, max_depth, &mut mate_move, prev_hash) {
        Some(mate_move)
    } else {
        None
    }
}

// 詰み探索の本体（再帰関数）
fn find_mate_recursive(
    vidro: &mut Bitboard,
    depth: usize,
    mate_move: &mut MoveBit,
    prev_hash: Option<u64>,
) -> bool {
    //深さ切れ(詰みなしと判断)
    if depth == 0 {
        return false;
    }

    let hash = vidro.to_compression_bod();

    let attacking_moves = generate_threat_moves(vidro, prev_hash);
    if attacking_moves.is_empty() {
        return false; //詰めろを掛けられない
    }

    //OR探索
    for mv in attacking_moves {
        if vidro
            .apply_force_with_check_illegal_move(mv, prev_hash)
            .is_err()
        {
            continue;
        }

        //受けが無くなっているかどうかを調べる
        if check_opponent_defense(vidro, depth - 1, mate_move, Some(hash)) {
            //受けがないことが確定 == 詰みが見つかった
            vidro.undo_force(mv);
            *mate_move = mv.clone(); //最後の代入の値==最初に指す手==詰み手順に入る時の手
            return true;
        }
        vidro.undo_force(mv);
    }

    //どの手も詰みにならなかった
    false
}

//NOTE! 詰みの読み筋を相手の物も含めるようにする

//受けがないかどうか
fn check_opponent_defense(
    vidro: &mut Bitboard,
    depth: usize,
    mate_move: &mut MoveBit,
    prev_hash: Option<u64>,
) -> bool {
    //勝になっていないかを確認
    if let EvalValue::Win(v) = vidro.win_eval().value {
        if v as i8 == -vidro.turn {
            return true;
        }
    }

    if depth == 0 {
        return false;
    }

    let hash = vidro.to_compression_bod();

    let defending_moves = vidro.generate_legal_move();
    if defending_moves.is_empty() {
        //受けなし
        return true;
    }

    // 生成した受け手の全ての応手に対して、詰み手順が続くか調べる (AND検索)
    for mv in defending_moves {
        if vidro
            .apply_force_with_check_illegal_move(mv, prev_hash)
            .is_err()
        {
            continue;
        }

        // 自分が再度攻めて詰むかどうかを再帰的に調べる
        let can_mate = find_mate_recursive(vidro, depth - 1, mate_move, Some(hash));

        vidro.undo_force(mv);

        if !can_mate {
            // 相手のこの受けで詰みが途切れた。
            // したがって、元の自分の手は必勝の詰み手順ではない。
            return false;
        }
    }

    // 相手がどう受けても、全て詰み手順が続くことが証明された
    true
}

pub fn is_reach(vidro: &mut Bitboard, prev_hash: Option<u64>) -> bool {
    vidro.turn_change(); //意図的に手番を書き換え2手差しさせたときに勝利することがあるかを調べる
    let result = checkmate_in_one_move(vidro, prev_hash);
    vidro.turn_change(); //手番を戻す
    result
}

pub fn checkmate_in_one_move(vidro: &mut Bitboard, prev_hash: Option<u64>) -> bool {
    let moves = vidro.generate_legal_move_only_flick(None);
    let turn = vidro.turn;
    for &mv in &moves {
        if vidro
            .apply_force_with_check_illegal_move(mv, prev_hash)
            .is_err()
        {
            continue;
        }
        if let EvalValue::Win(value) = vidro.win_eval().value {
            if value as i8 == turn {
                vidro.undo_force(mv);
                return true;
            }
        }
        vidro.undo_force(mv);
    }
    false
}

pub fn generate_threat_moves(vidro: &mut Bitboard, prev_hash: Option<u64>) -> Vec<MoveBit> {
    let mut moves = vidro.generate_legal_move();
    let hash = vidro.to_compression_bod();
    moves.retain(|&mv| {
        if vidro
            .apply_force_with_check_illegal_move(mv, prev_hash)
            .is_err()
        {
            return false;
        }
        //詰めろ(自殺手を除く)
        if is_reach(vidro, prev_hash) && !checkmate_in_one_move(vidro, Some(hash)) {
            vidro.undo_force(mv);
            return true;
        }
        vidro.undo_force(mv);
        false
    });
    moves
}
