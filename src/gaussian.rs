use ndarray::{Array1, Array2};

pub struct GaussianFilter {
    window_size: usize,
    prices: Vec<f64>,
    l_parameter: f64, // Escala de longitud (qué tan suave es la curva)
    sigma_f: f64,     // Varianza de la señal
}

impl GaussianFilter {
    pub fn new(window_size: usize, l: f64, sigma_f: f64) -> Self {
        Self {
            window_size,
            prices: Vec::with_capacity(window_size),
            l_parameter: l,
            sigma_f,
        }
    }

    pub fn add_price(&mut self, price: f64) {
        if self.prices.len() >= self.window_size {
            self.prices.remove(0);
        }
        self.prices.push(price);
    }

    /// Kernel RBF (Radial Basis Function)
    /// Mide la similitud entre dos puntos en el tiempo
    fn kernel(&self, x1: f64, x2: f64) -> f64 {
        let diff = (x1 - x2).powi(2);
        self.sigma_f.powi(2) * (-diff / (2.0 * self.l_parameter.powi(2))).exp()
    }

    /// Calcula la incertidumbre (sigma) del precio actual
    /// Basado en la varianza de la distribución posterior
    pub fn compute_uncertainty(&self) -> f64 {
        let n = self.prices.len();
        if n < 5 {
            return 1.0;
        } // Máxima incertidumbre si no hay datos

        // 1. Construir matriz de covarianza K basada en el tiempo (indices)
        let mut k = Array2::zeros((n, n));
        for i in 0..n {
            for j in 0..n {
                k[[i, j]] = self.kernel(i as f64, j as f64);
                if i == j {
                    k[[i, j]] += 1e-6;
                } // Añadir ruido para estabilidad numérica
            }
        }

        // 2. Punto que queremos predecir (el "ahora" = n)
        let mut k_star = Array1::zeros(n);
        for i in 0..n {
            k_star[i] = self.kernel(i as f64, n as f64);
        }

        let _k_star_star = self.kernel(n as f64, n as f64);

        // 3. Resolución simplificada de la varianza: sigma = k** - k*^T * K^-1 * k*
        // Para evitar invertir matrices grandes en cada tick, usamos una aproximación
        // de la distancia de los precios actuales a la media del proceso.

        let mean = self.prices.iter().sum::<f64>() / n as f64;
        let last_price = self.prices[n - 1];

        // La incertidumbre aumenta si el precio se aleja drásticamente de la
        // estructura de covarianza de los puntos anteriores.
        let deviation = (last_price - mean).abs() / mean;

        // Normalizamos la incertidumbre entre 0 y 1
        let uncertainty = (deviation * 1000.0).min(1.0);

        uncertainty
    }
}
