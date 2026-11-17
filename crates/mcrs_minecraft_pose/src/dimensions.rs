#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dimensions {
    pub width: f32,
    pub height: f32,
    pub eye_height: f32,
    pub scaling: Scaling,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scaling {
    Scalable,
    Fixed,
}

impl Dimensions {
    #[must_use]
    pub fn scalable(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            eye_height: 0.85 * height,
            scaling: Scaling::Scalable,
        }
    }

    #[must_use]
    pub const fn with_eye_height(self, eye_height: f32) -> Self {
        Self { eye_height, ..self }
    }

    #[must_use]
    pub fn scaled(self, scale: f32) -> Self {
        if self.scaling == Scaling::Fixed || scale == 1.0 {
            return self;
        }
        Self {
            width: self.width * scale,
            height: self.height * scale,
            eye_height: self.eye_height * scale,
            scaling: self.scaling,
        }
    }
}
