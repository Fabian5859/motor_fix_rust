use ndarray::{Array1, Array2};
use ndarray_rand::RandomExt;
use rand_distr::Normal;
use std::f64::consts::E;

pub struct BayesianBrain {
    // Pesos (W1: Input -> Hidden, W2: Hidden -> Output)
    weights1: Array2<f64>,
    weights2: Array1<f64>,
    // Incertidumbre de los pesos (Varianza)
    variance1: Array2<f64>,
    variance2: Array1<f64>,
    learning_rate: f64,
}

impl BayesianBrain {
    pub fn new(input_dim: usize, hidden_dim: usize, lr: f64) -> Self {
        Self {
            weights1: Array2::random((input_dim, hidden_dim), Normal::new(0.0, 0.1).unwrap()),
            weights2: Array1::random(hidden_dim, Normal::new(0.0, 0.1).unwrap()),
            variance1: Array2::from_elem((input_dim, hidden_dim), 0.05),
            variance2: Array1::from_elem(hidden_dim, 0.05),
            learning_rate: lr,
        }
    }

    /// Activación Sigmoide
    fn sigmoid(&self, x: f64) -> f64 {
        1.0 / (1.0 + E.powf(-x))
    }

    /// Derivada de Sigmoide para Backpropagation
    fn sigmoid_derivative(&self, x: f64) -> f64 {
        let s = self.sigmoid(x);
        s * (1.0 - s)
    }

    /// Forward pass que devuelve (Predicción, Incertidumbre de la Red)
    pub fn predict_with_uncertainty(&self, inputs: &Array1<f64>) -> (f64, f64) {
        if inputs.is_empty() {
            return (0.5, 1.0);
        }

        // Capa Oculta
        let z1 = inputs.dot(&self.weights1);
        let a1 = z1.mapv(|x| self.sigmoid(x));

        // Salida
        let z2 = a1.dot(&self.weights2);
        let prediction = self.sigmoid(z2);

        // Cálculo de Incertidumbre Epistémica (basado en la varianza de los pesos)
        // A mayor varianza en los pesos internos, mayor incertidumbre en la salida
        let uncertainty = self.variance1.sum() * 0.1 + self.variance2.sum() * 0.9;

        (prediction, uncertainty.clamp(0.0, 1.0))
    }

    /// Entrenamiento Online (Backpropagation Bayesiano simplificado)
    pub fn train(&mut self, inputs: &Array1<f64>, target: f64) {
        if inputs.is_empty() {
            return;
        }

        // 1. Forward
        let z1 = inputs.dot(&self.weights1);
        let a1 = z1.mapv(|x| self.sigmoid(x));
        let z2 = a1.dot(&self.weights2);
        let prediction = self.sigmoid(z2);

        // 2. Cálculo del Error
        let error = prediction - target;

        // 3. Backpropagation para W2
        let d_w2 = error * self.sigmoid_derivative(z2);
        for i in 0..self.weights2.len() {
            let grad = d_w2 * a1[i];
            self.weights2[i] -= self.learning_rate * grad;
            // Reducimos la varianza (ganamos confianza) si el error es pequeño
            self.variance2[i] *= 0.99 + (error.abs() * 0.01);
        }

        // 4. Backpropagation para W1
        let d_z1 = d_w2 * &self.weights2;
        for i in 0..self.weights1.nrows() {
            for j in 0..self.weights1.ncols() {
                let grad = d_z1[j] * self.sigmoid_derivative(z1[j]) * inputs[i];
                self.weights1[[i, j]] -= self.learning_rate * grad;
                self.variance1[[i, j]] *= 0.99 + (error.abs() * 0.01);
            }
        }
    }
}
