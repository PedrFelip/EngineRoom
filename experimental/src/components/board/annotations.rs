//! User drawings, separate from engine arrows.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Brush {
    Green,
    Red,
    Blue,
    Yellow,
}

impl Brush {
    pub(crate) fn from_modifiers(shift_or_ctrl: bool, alt_or_meta: bool) -> Self {
        match (shift_or_ctrl, alt_or_meta) {
            (false, false) => Self::Green,
            (true, false) => Self::Red,
            (false, true) => Self::Blue,
            (true, true) => Self::Yellow,
        }
    }

    pub(crate) fn color(self) -> rune::Color {
        let hex = match self {
            Self::Green => "#15781b",
            Self::Red => "#882020",
            Self::Blue => "#003088",
            Self::Yellow => "#e68f00",
        };
        rune::color::hex(hex).expect("annotation brush colors are valid hex")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Annotation {
    pub(crate) from: usize,
    /// The same square represents a circle.
    pub(crate) to: usize,
    pub(crate) brush: Brush,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Annotations {
    shapes: Vec<Annotation>,
}

impl Annotations {
    pub(crate) fn shapes(&self) -> &[Annotation] {
        &self.shapes
    }

    /// Repeating a shape removes it; changing its brush replaces it.
    pub(crate) fn toggle(&mut self, shape: Annotation) {
        if let Some(index) = self
            .shapes
            .iter()
            .position(|s| s.from == shape.from && s.to == shape.to)
        {
            let previous = self.shapes.remove(index);
            if previous.brush == shape.brush {
                return;
            }
        }
        self.shapes.push(shape);
    }

    pub(crate) fn clear(&mut self) {
        self.shapes.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_toggle_and_change_color_independently() {
        let mut drawings = Annotations::default();
        let circle = Annotation {
            from: 12,
            to: 12,
            brush: Brush::Green,
        };
        let arrow = Annotation {
            from: 12,
            to: 28,
            brush: Brush::Blue,
        };
        drawings.toggle(circle);
        drawings.toggle(arrow);
        drawings.toggle(Annotation {
            brush: Brush::Red,
            ..circle
        });
        assert_eq!(
            drawings.shapes(),
            &[
                arrow,
                Annotation {
                    brush: Brush::Red,
                    ..circle
                }
            ]
        );
        drawings.toggle(Annotation {
            brush: Brush::Red,
            ..circle
        });
        assert_eq!(drawings.shapes(), &[arrow]);
        drawings.clear();
        assert!(drawings.shapes().is_empty());
    }

    #[test]
    fn secondary_click_toggles_a_circle_without_moving_a_piece() {
        let mut rt = rune::App::new(crate::spec()).headless_runtime(
            (900.0, 700.0),
            rune::Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.state = crate::state::State::fixture();
        rt.state.page = crate::state::Page::Review;
        rt.invalidate();
        rt.frame(0.0);
        let (x, y) = rt.find("square-e2").expect("e2 is visible on the board");
        rt.secondary_down(x, y);
        rt.secondary_up(x, y);
        rt.frame(1.0 / 60.0);
        assert_eq!(
            rt.state.board_annotations.shapes(),
            &[Annotation {
                from: 52,
                to: 52,
                brush: Brush::Green
            }]
        );
        assert!(rt.state.variations.is_empty());
        rt.secondary_down(x, y);
        rt.secondary_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.board_annotations.shapes().is_empty());
    }

    #[test]
    fn modifier_combinations_match_chessground() {
        assert_eq!(Brush::from_modifiers(false, false), Brush::Green);
        assert_eq!(Brush::from_modifiers(true, false), Brush::Red);
        assert_eq!(Brush::from_modifiers(false, true), Brush::Blue);
        assert_eq!(Brush::from_modifiers(true, true), Brush::Yellow);
    }
}
