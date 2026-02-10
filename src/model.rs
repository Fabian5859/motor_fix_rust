use ndarray::Array1;
use std::f64::consts::E;

pub struct LogisticModel {
    pub weights: Array1<f64>,
    pub bias: f64,
    pub learning_rate: f64,
}

impl LogisticModel {
    /// Inicializa el modelo con pesos en cero o valores aleatorios pequeños
    pub fn new(num_features: usize, learning_rate: f64) -> Self {
        Self {
            weights: Array1::zeros(num_features),
            bias: 0.0,
            learning_rate,
        }
    }

    /// Función Sigmoide: Mapea cualquier valor a un rango (0, 1)
    /// Representa la probabilidad de que el precio suba.
    fn sigmoid(&self, z: f64) -> f64 {
        1.0 / (1.0 + E.powf(-z))
    }

    /// Predice la probabilidad de movimiento alcista (0.0 a 1.0)
    pub fn predict(&self, features: &Array1<f64>) -> f64 {
        let z = self.weights.dot(features) + self.bias;
        self.sigmoid(z)
    }

    /// Entrena el modelo con una sola muestra (Online Learning)
    /// target: 1.0 si el precio subió, 0.0 si bajó.
    pub fn train(&mut self, features: &Array1<f64>, target: f64) -> f64 {
        // 1. Obtener predicción actual
        let prediction = self.predict(features);
        
        // 2. Calcular el error (Gradiente)
        let error = prediction - target;

        // 3. Actualizar pesos: W = W - (LR * error * X)
        // Usamos la derivada de la función de pérdida log-likelihood
        let gradient_w = features.mapv(|x| x * error);
        self.weights = &self.weights - &(gradient_w * self.learning_rate);
        
        // 4. Actualizar bias
        self.bias -= self.learning_rate * error;

        // Retornamos el error cuadrático para monitorear el aprendizaje
        error.powi(2)
    }
}
