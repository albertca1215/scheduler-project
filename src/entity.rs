use macroquad::prelude::{Color, Vec2};

/// State for one moving object in the simulation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entity {
    pub position: Vec2,
    pub velocity: Vec2,
    pub radius: f32,
    pub color: Color,
}

impl Entity {
    #[must_use]
    pub fn new(position: Vec2, velocity: Vec2, radius: f32, color: Color) -> Self {
        Self {
            position,
            velocity,
            radius,
            color,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::prelude::{vec2, WHITE};

    #[test]
    fn entity_stores_initial_simulation_state() {
        let entity = Entity::new(vec2(100.0, 80.0), vec2(30.0, -20.0), 15.0, WHITE);

        assert_eq!(entity.position, vec2(100.0, 80.0));
        assert_eq!(entity.velocity, vec2(30.0, -20.0));
        assert_eq!(entity.radius, 15.0);
        assert_eq!(entity.color, WHITE);
    }
}
