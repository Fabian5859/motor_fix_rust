use crate::state::OrderBook;
use ndarray::{Array1, Array2};

pub struct FeatureCollector {
    pub window_size: usize,
    pub data: Vec<Vec<f64>>,
    pub means: Array1<f64>,
    pub stds: Array1<f64>,
}

impl FeatureCollector {
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size,
            data: Vec::new(),
            // Iniciamos con 10 dimensiones (Price, Vel, Ruido, Contexto + 3 Bids + 3 Asks)
            // Esto se ajustará dinámicamente al primer vector real
            means: Array1::zeros(0),
            stds: Array1::ones(0),
        }
    }

    /// Empaqueta todas las señales en un solo vector de entrada
    pub fn push_features(&mut self, book: &OrderBook, velocity: f64, noise: f64, context: f64) {
        let mut current_row = Vec::new();

        // 1. Precio medio (Normalizado internamente luego)
        current_row.push(book.get_mid_price().unwrap_or(0.0));

        // 2. Dinámica del mercado
        current_row.push(velocity);
        current_row.push(noise);
        current_row.push(context);

        // 3. Profundidad del Libro (Niveles 1 a 3)
        // Esto captura la "geometría" del LOB
        let depth_v = book.get_depth_vector(3);
        current_row.extend(depth_v);

        // Gestión de la ventana deslizante para normalización
        if self.data.len() >= self.window_size {
            self.data.remove(0);
        }
        self.data.push(current_row);

        // Actualizamos estadísticas de normalización si tenemos datos suficientes
        if self.data.len() >= 10 {
            self.update_stats();
        }
    }

    fn update_stats(&mut self) {
        let rows = self.data.len();
        let cols = self.data[0].len();

        let mut matrix = Array2::zeros((rows, cols));
        for (i, row) in self.data.iter().enumerate() {
            for (j, &val) in row.iter().enumerate() {
                matrix[[i, j]] = val;
            }
        }

        self.means = matrix.mean_axis(ndarray::Axis(0)).unwrap();
        self.stds = matrix.std_axis(ndarray::Axis(0), 0.0);

        // Evitar división por cero
        self.stds.mapv_inplace(|x| if x == 0.0 { 1.0 } else { x });
    }

    /// Devuelve el último vector transformado para la Red Neuronal
    pub fn get_standardized_vector(&self) -> Array1<f64> {
        if self.data.is_empty() || self.means.is_empty() {
            return Array1::zeros(0);
        }

        let last_raw = Array1::from_vec(self.data.last().unwrap().clone());
        (last_raw - &self.means) / &self.stds
    }
}

