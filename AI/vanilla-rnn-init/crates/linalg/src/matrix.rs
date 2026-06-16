use crate::Vector;

/// 실수 행렬. 생성 시 직사각형(모든 행의 길이 동일)임을 검증한다.
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<Vec<f64>>,
}

impl Matrix {
    /// 행 단위 데이터로 행렬을 만든다. 행 길이가 들쭉날쭉하면 패닉.
    pub fn new(data: Vec<Vec<f64>>) -> Self {
        let rows = data.len();
        let cols = data.first().map_or(0, Vec::len);
        for (i, row) in data.iter().enumerate() {
            assert_eq!(row.len(), cols, "Matrix::new 행 {i} 길이 불일치: {} (기대 {cols})", row.len());
        }
        Self {
            rows,
            cols,
            data,
        }
    }

    /// `rows × cols` 영행렬.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![vec![0.0; cols]; rows],
        }
    }

    /// `(i, j)`마다 `f`를 호출해 `rows × cols` 행렬을 만든다.
    pub fn from_fn<F: FnMut(usize, usize) -> f64>(rows: usize, cols: usize, mut f: F) -> Self {
        let data = (0 .. rows).map(|i| (0 .. cols).map(|j| f(i, j)).collect()).collect();
        Self {
            rows,
            cols,
            data,
        }
    }

    pub fn rows(&self) -> usize { self.rows }

    pub fn cols(&self) -> usize { self.cols }

    /// `(i, j)` 성분. 범위를 벗어나면 패닉.
    pub fn get(&self, i: usize, j: usize) -> f64 { self.data[i][j] }

    /// 행렬-벡터 곱 `self * v`. `self.cols != v.len()`이면 패닉.
    pub fn vec_mul(&self, v: &Vector) -> Vector {
        assert_eq!(self.cols, v.len(), "Matrix::vec_mul 차원 불일치: cols={} vs len={}", self.cols, v.len());
        Vector::from_fn(self.rows, |i| self.data[i].iter().zip(v.iter()).map(|(a, b)| a * b).sum())
    }

    /// 전치 행렬.
    pub fn transpose(&self) -> Matrix { Matrix::from_fn(self.cols, self.rows, |i, j| self.data[j][i]) }

    /// 성분별 덧셈. 차원이 다르면 패닉.
    pub fn add(&self, other: &Matrix) -> Matrix {
        assert!(
            self.rows == other.rows && self.cols == other.cols,
            "Matrix::add 차원 불일치: {}x{} vs {}x{}",
            self.rows,
            self.cols,
            other.rows,
            other.cols
        );
        Matrix::from_fn(self.rows, self.cols, |i, j| self.data[i][j] + other.data[i][j])
    }

    /// 스칼라 곱.
    pub fn scale(&self, s: f64) -> Matrix { Matrix::from_fn(self.rows, self.cols, |i, j| s * self.data[i][j]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_infers_shape() {
        let m = Matrix::new(vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]);
        assert_eq!((m.rows(), m.cols()), (2, 3));
        assert_eq!(m.get(1, 2), 6.0);
    }

    #[test]
    #[should_panic(expected = "행 1 길이 불일치")]
    fn new_jagged_panics() { let _ = Matrix::new(vec![vec![1.0, 2.0], vec![3.0]]); }

    #[test]
    fn vec_mul_matches_hand_calc() {
        let m = Matrix::new(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        let v = Vector::new(vec![5.0, 6.0]);
        // [1*5+2*6, 3*5+4*6] = [17, 39]
        assert_eq!(m.vec_mul(&v).as_slice(), &[17.0, 39.0]);
    }

    #[test]
    #[should_panic(expected = "차원 불일치")]
    fn vec_mul_dim_mismatch_panics() {
        let m = Matrix::new(vec![vec![1.0, 2.0]]);
        let v = Vector::new(vec![1.0, 2.0, 3.0]);
        let _ = m.vec_mul(&v);
    }

    #[test]
    fn transpose_swaps_dims() {
        let m = Matrix::new(vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]);
        let t = m.transpose();
        assert_eq!((t.rows(), t.cols()), (3, 2));
        assert_eq!(t.get(2, 0), 3.0);
        assert_eq!(t.get(0, 1), 4.0);
    }

    #[test]
    fn add_and_scale() {
        let a = Matrix::new(vec![vec![1.0, 2.0]]);
        let b = Matrix::new(vec![vec![3.0, 4.0]]);
        assert_eq!(a.add(&b).get(0, 1), 6.0);
        assert_eq!(a.scale(-2.0).get(0, 0), -2.0);
    }

    #[test]
    #[should_panic(expected = "차원 불일치")]
    fn add_dim_mismatch_panics() {
        let a = Matrix::zeros(2, 2);
        let b = Matrix::zeros(2, 3);
        let _ = a.add(&b);
    }
}
