use std::collections::VecDeque;

pub struct PriceBuffer {
    capacity: usize,
    prices: VecDeque<f64>,
}

impl PriceBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            prices: VecDeque::with_capacity(capacity),
        }
    }

    pub fn add_price(&mut self, price: f64) {
        if self.prices.len() >= self.capacity {
            self.prices.pop_front(); // Eliminamos el precio más viejo
        }
        self.prices.push_back(price); // Añadimos el más reciente
    }

    pub fn get_count(&self) -> usize {
        self.prices.len()
    }

    // Preparado para la Fase 3: devuelve los precios para cálculos
    pub fn get_prices(&self) -> &VecDeque<f64> {
        &self.prices
    }
}
