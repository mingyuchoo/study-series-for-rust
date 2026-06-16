use crate::{backward::RNNGradients,
            model::RNNParams};

/// 확률적 경사 하강법(SGD) 한 스텝: `θ ← θ - lr · ∇θ`.
pub fn update_params(lr: f64, params: &RNNParams, grads: &RNNGradients) -> RNNParams {
    RNNParams {
        wxh: params.wxh.add(&grads.dwxh.scale(-lr)),
        whh: params.whh.add(&grads.dwhh.scale(-lr)),
        why: params.why.add(&grads.dwhy.scale(-lr)),
        bh: params.bh.add(&grads.dbh.scale(-lr)),
        by: params.by.add(&grads.dby.scale(-lr)),
    }
}
