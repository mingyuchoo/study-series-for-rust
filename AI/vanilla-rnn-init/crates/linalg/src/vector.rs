use crate::Matrix;
use std::ops::Index;

/// 실수 벡터. 내부 표현(`Vec<f64>`)을 캡슐화한다.
#[derive(Debug, Clone, PartialEq)]
pub struct Vector {
    data: Vec<f64>,
}

impl Vector {
    /// 주어진 성분으로 벡터를 만든다.
    pub fn new(data: Vec<f64>) -> Self {
        Self {
            data,
        }
    }

    /// 길이 `n`의 영벡터.
    pub fn zeros(n: usize) -> Self {
        Self {
            data: vec![0.0; n],
        }
    }

    /// 인덱스마다 `f`를 호출해 길이 `n`의 벡터를 만든다.
    pub fn from_fn<F: FnMut(usize) -> f64>(n: usize, f: F) -> Self {
        Self {
            data: (0 .. n).map(f).collect(),
        }
    }

    pub fn len(&self) -> usize { self.data.len() }

    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    pub fn as_slice(&self) -> &[f64] { &self.data }

    pub fn iter(&self) -> std::slice::Iter<'_, f64> { self.data.iter() }

    /// 성분별로 `f`를 적용한 새 벡터.
    pub fn map<F: FnMut(f64) -> f64>(&self, mut f: F) -> Vector { Vector::new(self.data.iter().map(|&x| f(x)).collect()) }

    /// 성분별 덧셈. 길이가 다르면 패닉.
    pub fn add(&self, other: &Vector) -> Vector {
        self.assert_same_len(other, "add");
        self.zip_map(other, |a, b| a + b)
    }

    /// 성분별 뺄셈. 길이가 다르면 패닉.
    pub fn sub(&self, other: &Vector) -> Vector {
        self.assert_same_len(other, "sub");
        self.zip_map(other, |a, b| a - b)
    }

    /// 스칼라 곱.
    pub fn scale(&self, s: f64) -> Vector { self.map(|x| s * x) }

    /// 성분별(아다마르) 곱. 길이가 다르면 패닉.
    pub fn hadamard(&self, other: &Vector) -> Vector {
        self.assert_same_len(other, "hadamard");
        self.zip_map(other, |a, b| a * b)
    }

    /// 외적(outer product): `self ⊗ other`. 결과는 `self.len() × other.len()`
    /// 행렬.
    pub fn outer(&self, other: &Vector) -> Matrix { Matrix::from_fn(self.len(), other.len(), |i, j| self.data[i] * other.data[j]) }

    fn zip_map<F: FnMut(f64, f64) -> f64>(&self, other: &Vector, mut f: F) -> Vector {
        Vector::new(self.data.iter().zip(other.data.iter()).map(|(&a, &b)| f(a, b)).collect())
    }

    fn assert_same_len(&self, other: &Vector, op: &str) {
        assert_eq!(self.len(), other.len(), "Vector::{op} 길이 불일치: {} vs {}", self.len(), other.len());
    }
}

impl Index<usize> for Vector {
    type Output = f64;

    fn index(&self, i: usize) -> &f64 { &self.data[i] }
}

impl From<Vec<f64>> for Vector {
    fn from(data: Vec<f64>) -> Self {
        Self {
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeros_and_len() {
        let v = Vector::zeros(3);
        assert_eq!(v.len(), 3);
        assert_eq!(v.as_slice(), &[0.0, 0.0, 0.0]);
    }

    #[test]
    fn from_fn_indexes() {
        let v = Vector::from_fn(4, |i| i as f64 * 2.0);
        assert_eq!(v.as_slice(), &[0.0, 2.0, 4.0, 6.0]);
    }

    #[test]
    fn elementwise_ops() {
        let a = Vector::new(vec![1.0, 2.0, 3.0]);
        let b = Vector::new(vec![4.0, 5.0, 6.0]);
        assert_eq!(a.add(&b).as_slice(), &[5.0, 7.0, 9.0]);
        assert_eq!(b.sub(&a).as_slice(), &[3.0, 3.0, 3.0]);
        assert_eq!(a.scale(2.0).as_slice(), &[2.0, 4.0, 6.0]);
        assert_eq!(a.hadamard(&b).as_slice(), &[4.0, 10.0, 18.0]);
    }

    #[test]
    fn outer_product_shape_and_values() {
        let a = Vector::new(vec![1.0, 2.0]);
        let b = Vector::new(vec![3.0, 4.0, 5.0]);
        let m = a.outer(&b);
        assert_eq!((m.rows(), m.cols()), (2, 3));
        assert_eq!(m.get(1, 2), 2.0 * 5.0);
    }

    #[test]
    #[should_panic(expected = "길이 불일치")]
    fn add_mismatched_len_panics() {
        let a = Vector::new(vec![1.0, 2.0]);
        let b = Vector::new(vec![1.0]);
        let _ = a.add(&b);
    }
}
