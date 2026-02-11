#[derive(Debug)]
pub enum MarketState {
    Low,
    Normal,
    High,
}

pub struct BayesianNetwork {
    pub context_threshold: f64,
}

impl BayesianNetwork {
    pub fn new(threshold: f64) -> Self {
        Self {
            context_threshold: threshold,
        }
    }

    fn discretize_spread(&self, spread: f64) -> MarketState {
        if spread < 1.5 {
            MarketState::Low
        } else if spread < 4.0 {
            MarketState::Normal
        } else {
            MarketState::High
        }
    }

    fn discretize_velocity(&self, velocity: f64) -> MarketState {
        if velocity < 5.0 {
            MarketState::Low
        } else if velocity < 25.0 {
            MarketState::Normal
        } else {
            MarketState::High
        }
    }

    /// Nueva discretización para la Intensidad (Liquidez)
    fn discretize_intensity(&self, intensity: f64) -> MarketState {
        if intensity < 100000.0 {
            MarketState::Low
        } else if intensity < 1000000.0 {
            MarketState::Normal
        } else {
            MarketState::High
        }
    }

    pub fn compute_context_score(
        &self,
        spread: f64,
        velocity: f64,
        imbalance: f64,
        intensity: f64,
    ) -> f64 {
        let s_state = self.discretize_spread(spread);
        let v_state = self.discretize_velocity(velocity);
        let i_state = self.discretize_intensity(intensity);

        let mut score: f64 = 0.5; // Punto de partida neutral

        // REGLA 1: Spread (Costo de entrada/salida)
        match s_state {
            MarketState::Low => score += 0.15,
            MarketState::Normal => score += 0.05,
            MarketState::High => score -= 0.25,
        }

        // REGLA 2: Velocidad (Actividad del mercado)
        match v_state {
            MarketState::Low => score -= 0.15, // Demasiado lento, sin momentum
            MarketState::Normal => score += 0.10, // Actividad ideal
            MarketState::High => score -= 0.10, // Posible sobre-reacción o ruido
        }

        // REGLA 3: Intensidad (Respaldo de la Liquidez)
        match i_state {
            MarketState::Low => score -= 0.20, // Libro "delgado", peligro de manipulación
            MarketState::Normal => score += 0.05,
            MarketState::High => score += 0.15, // Mercado institucional profundo
        }

        // REGLA CAUSAL COMBINADA: Imbalance en libro vacío es falso
        if imbalance.abs() > 0.7 && intensity < 200000.0 {
            score -= 0.20;
        }

        score.clamp(0.0, 1.0)
    }

    pub fn is_context_favorable(&self, score: f64) -> bool {
        score >= self.context_threshold
    }
}
