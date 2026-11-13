use libm::floor;

use crate::gui_rect::GuiRect;
use crate::saturate;
use crate::standard_sizes::SLIDER_HANDLE_WIDTH;
use crate::widget_event::WidgetEvent;
use crate::widget_id::WidgetId;

const DEFERRED_APPLY_MS: u64 = 600;
const LAST_POSITION: f64 = 0.999_99_f32 as f64;

/// The position of a slider from 0 to 1 and the option values it maps to (WID-SLD-004 to
/// WID-SLD-007).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slider {
    position: f64,
    scale: Scale,
    deferred: bool,
    last_change_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Scale {
    Continuous,
    Range { min: i32, max: i32 },
    List { count: usize },
}

pub(crate) enum SliderStep {
    Down,
    Up,
}

impl Slider {
    /// A slider whose value is its position, clamped to [0, 1].
    #[must_use]
    pub fn continuous(position: f64) -> Self {
        Self::at(Scale::Continuous, position)
    }

    /// An option slider over the integers `min` to `max` at `value`; none unless `min < max`
    /// (WID-SLD-005).
    #[must_use]
    pub fn range(min: i32, max: i32, value: i32) -> Option<Self> {
        if min >= max {
            return None;
        }
        let scale = Scale::Range { min, max };
        Some(Self::at(scale, scale.position_of(f64::from(value))))
    }

    /// An option slider over a list of `count` values at the index `index`; none unless
    /// `index < count` and the list has two values or more (WID-SLD-006).
    #[must_use]
    pub fn list(count: usize, index: usize) -> Option<Self> {
        if count < 2 || index >= count {
            return None;
        }
        let scale = Scale::List { count };
        Some(Self::at(scale, scale.position_of(index as f64)))
    }

    /// The same slider for an option that stores its value only 600 ms after the last change
    /// (WID-SLD-007).
    #[must_use]
    pub const fn deferred(self) -> Self {
        Self {
            deferred: true,
            ..self
        }
    }

    /// The position of the handle, from 0 to 1.
    #[must_use]
    pub const fn position(&self) -> f64 {
        self.position
    }

    /// The integer value of a range slider at its position.
    #[must_use]
    pub fn value(&self) -> Option<i32> {
        match self.scale {
            Scale::Range { .. } => Some(self.option_value() as i32),
            Scale::Continuous | Scale::List { .. } => None,
        }
    }

    /// The index of a list slider at its position.
    #[must_use]
    pub fn index(&self) -> Option<usize> {
        match self.scale {
            Scale::List { .. } => Some(self.option_value() as usize),
            Scale::Continuous | Scale::Range { .. } => None,
        }
    }

    pub(crate) const fn is_deferred(&self) -> bool {
        self.deferred
    }

    pub(crate) fn handle(&self, bounds: GuiRect) -> GuiRect {
        let travel = i64::from(bounds.width) - i64::from(SLIDER_HANDLE_WIDTH);
        let offset = (self.position * travel as f64) as i64;
        GuiRect::new(
            saturate::saturate(i64::from(bounds.x) + offset),
            bounds.y,
            SLIDER_HANDLE_WIDTH,
            bounds.height,
        )
    }

    pub(crate) fn move_to(
        &mut self,
        widget: WidgetId,
        position: f64,
        now_ms: u64,
    ) -> Option<WidgetEvent> {
        let position = clamp_unit(position);
        if position == self.position {
            return None;
        }
        self.position = position;
        if self.deferred {
            self.last_change_ms = Some(now_ms);
            Some(WidgetEvent::ValuePending { widget })
        } else {
            Some(WidgetEvent::ValueChanged { widget })
        }
    }

    pub(crate) fn step(
        &mut self,
        widget: WidgetId,
        step: SliderStep,
        bounds: GuiRect,
        now_ms: u64,
    ) -> Option<WidgetEvent> {
        let sign = match step {
            SliderStep::Down => -1.0,
            SliderStep::Up => 1.0,
        };
        let target = match self.scale {
            Scale::Continuous => self.position + sign / travel(bounds),
            Scale::Range { .. } | Scale::List { .. } => {
                self.scale.position_of(self.option_value() + sign)
            }
        };
        self.move_to(widget, target, now_ms)
    }

    pub(crate) fn snap(&mut self) {
        self.position = clamp_unit(self.scale.position_of(self.option_value()));
    }

    pub(crate) fn apply_due(&mut self, now_ms: u64) -> bool {
        let Some(changed) = self.last_change_ms else {
            return false;
        };
        if now_ms.saturating_sub(changed) < DEFERRED_APPLY_MS {
            return false;
        }
        self.last_change_ms = None;
        self.snap();
        true
    }

    fn at(scale: Scale, position: f64) -> Self {
        Self {
            position: clamp_unit(position),
            scale,
            deferred: false,
            last_change_ms: None,
        }
    }

    fn option_value(&self) -> f64 {
        let unit = if self.position >= 1.0 {
            LAST_POSITION
        } else {
            self.position
        };
        match self.scale {
            Scale::Continuous => self.position,
            Scale::Range { min, max } => {
                floor(f64::from(min) + unit * (f64::from(max) + 1.0 - f64::from(min)))
            }
            Scale::List { count } => floor(unit * count as f64).min(count as f64 - 1.0),
        }
    }
}

impl Scale {
    fn position_of(self, value: f64) -> f64 {
        match self {
            Self::Continuous => value,
            Self::Range { min, max } => {
                let (min, max) = (f64::from(min), f64::from(max));
                if value == min {
                    0.0
                } else if value == max {
                    1.0
                } else {
                    (value + 0.5 - min) / (max + 1.0 - min)
                }
            }
            Self::List { count } => value / (count as f64 - 1.0),
        }
    }
}

pub(crate) fn position_at(bounds: GuiRect, x: f64) -> f64 {
    let half_handle = f64::from(SLIDER_HANDLE_WIDTH / 2);
    (x - (f64::from(bounds.x) + half_handle)) / travel(bounds)
}

fn travel(bounds: GuiRect) -> f64 {
    f64::from(bounds.width) - f64::from(SLIDER_HANDLE_WIDTH)
}

fn clamp_unit(position: f64) -> f64 {
    if position < 0.0 {
        0.0
    } else {
        position.min(1.0)
    }
}
