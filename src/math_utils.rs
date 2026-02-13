/// src/math_utils.rs
/// Utilidades matemáticas para gestión de riesgo Bayesiano y Cuantiles.

/// Función de error inversa (aproximación de Winitzki)
/// Necesaria para calcular cuantiles de una distribución Normal.
pub fn erf_inv(x: f64) -> f64 {
    let a = 0.147;
    let l = (1.0 - x * x).ln();
    let term1 = (2.0 / (std::f64::consts::PI * a)) + (l / 2.0);
    let term2 = l / a;

    let res = ((term1.powi(2) - term2).sqrt() - term1).sqrt();

    if x < 0.0 {
        -res
    } else {
        res
    }
}

/// Calcula el valor del cuantil (z-score) para una probabilidad p.
/// Ejemplo: p=0.95 devolverá ~1.645
pub fn normal_ppf(p: f64) -> f64 {
    std::f64::consts::SQRT_2 * erf_inv(2.0 * p - 1.0)
}

/// Calcula el Take Profit y Stop Loss dinámico basado en cuantiles.
/// Retorna (TP, SL)
pub fn calculate_bayesian_levels(
    mid_price: f64,
    mu: f64,
    sigma: f64,
    side: char,
    tp_percentile: f64, // ej: 0.75
    sl_percentile: f64, // ej: 0.25
) -> (f64, f64) {
    // mu en este contexto es el retorno esperado (ej: 0.0001 para 10 pips)
    // sigma es la volatilidad esperada

    let z_tp = normal_ppf(tp_percentile);
    let z_sl = normal_ppf(sl_percentile);

    if side == '1' {
        // LONG
        let tp = mid_price * (1.0 + mu + z_tp * sigma);
        let sl = mid_price * (1.0 + mu + z_sl * sigma);
        (tp, sl)
    } else {
        // SHORT
        // Para Short, el TP es un retorno negativo y el SL es un retorno positivo
        let tp = mid_price * (1.0 - (mu + z_tp * sigma));
        let sl = mid_price * (1.0 - (mu + z_sl * sigma));
        (tp, sl)
    }
}

/// Calcula el SNR (Signal-to-Noise Ratio)
pub fn calculate_snr(mu: f64, sigma: f64) -> f64 {
    if sigma == 0.0 {
        return 0.0;
    }
    (mu.abs()) / sigma
}
