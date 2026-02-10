use crate::state::OrderBook;
use ndarray::{Array1, Array2, Axis};

pub struct FeatureCollector {
    pub data_window: Array2<f64>,
    pub window_size: usize,
    current_idx: usize,
    is_full: bool, // Para saber si ya tenemos suficientes datos para normalizar
}

impl FeatureCollector {
    pub fn new(window_size: usize) -> Self {
        Self {
            data_window: Array2::zeros((window_size, 4)),
            window_size,
            current_idx: 0,
            is_full: false,
        }
    }

    pub fn push_features(&mut self, book: &OrderBook, velocity: f64, vol: f64) {
        let bid = book.get_best_bid().unwrap_or(0.0);
        let ask = book.get_best_ask().unwrap_or(0.0);

        let spread = (ask - bid).abs() * 100000.0;
        let imbalance = book.get_imbalance();

        let features = [imbalance, spread, velocity, vol];

        for (col, &value) in features.iter().enumerate() {
            self.data_window[[self.current_idx, col]] = value;
        }

        self.current_idx = (self.current_idx + 1) % self.window_size;
        if self.current_idx == 0 {
            self.is_full = true;
        }
    }

    /// Retorna el último vector normalizado basado en la media/desviación de la ventana
    pub fn get_standardized_vector(&self) -> Array1<f64> {
        let last_idx = if self.current_idx == 0 {
            self.window_size - 1
        } else {
            self.current_idx - 1
        };
        let raw_vector = self.data_window.row(last_idx);

        // Calculamos media y std dev por cada columna (Axis 0)
        let mean = self.data_window.mean_axis(Axis(0)).unwrap();
        let std = self.data_window.std_axis(Axis(0), 0.0);

        // Aplicamos Z-Score: (x - mean) / std
        // Añadimos un pequeño epsilon (1e-6) para evitar división por cero
        let mut standardized = (raw_vector.to_owned() - mean) / (std + 1e-6);

        standardized
    }

    pub fn get_last_vector(&self) -> Array1<f64> {
        let last_idx = if self.current_idx == 0 {
            self.window_size - 1
        } else {
            self.current_idx - 1
        };
        self.data_window.row(last_idx).to_owned()
    }
}

