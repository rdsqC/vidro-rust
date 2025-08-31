#[derive(Clone, Debug)]
pub enum EvalValue {
    Win(i16),
    Draw,    //探索的千日手などの引き合分け
    Unknown, //深さ不足で未確定
}

#[derive(Clone, Debug)]
pub struct Eval {
    pub value: EvalValue,
    pub evaluated: bool, //評価済みかどうか
}
