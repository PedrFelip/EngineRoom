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
    drawing: bool,
    origin: Option<usize>,
}

impl Annotations {
    pub(crate) fn drawing(&self) -> bool {
        self.drawing
    }
    pub(crate) fn origin(&self) -> Option<usize> {
        self.origin
    }
    pub(crate) fn toggle_drawing(&mut self) {
        self.drawing = !self.drawing;
        self.origin = None;
    }
    /// Two activations draw an arrow; activating the same square draws a circle.
    pub(crate) fn select_square(&mut self, square: usize) {
        if let Some(from) = self.origin.take() {
            self.toggle(Annotation {
                from,
                to: square,
                brush: Brush::Green,
            });
        } else {
            self.origin = Some(square);
        }
    }
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
        self.origin = None;
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
    fn drawing_mode_toggles_arrows_and_circles() {
        let mut drawings = Annotations::default();
        drawings.toggle_drawing();
        drawings.select_square(52);
        assert_eq!(drawings.origin(), Some(52));
        drawings.select_square(36);
        assert_eq!(
            drawings.shapes(),
            &[Annotation {
                from: 52,
                to: 36,
                brush: Brush::Green
            }]
        );
        drawings.select_square(52);
        drawings.select_square(36);
        assert!(drawings.shapes().is_empty());
        drawings.select_square(12);
        drawings.select_square(12);
        assert_eq!(drawings.shapes()[0].to, 12);
        drawings.select_square(3);
        drawings.toggle_drawing();
        assert_eq!(drawings.origin(), None);
    }

    #[test]
    fn drawing_controls_create_arrow_without_moving_a_piece() {
        let mut rt = rune::App::new(crate::spec()).headless_runtime(
            (720.0, 560.0),
            rune::Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.state = crate::state::State::fixture();
        rt.state.page = crate::state::Page::Review;
        rt.invalidate();
        rt.frame(0.0);
        for key in ["draw-annotations", "square-e2", "square-e4"] {
            let (x, y) = rt.find(key).expect("drawing control is visible");
            rt.pointer_down(x, y);
            rt.pointer_up(x, y);
            rt.frame(1.0 / 60.0);
        }
        assert_eq!(
            rt.state.board_annotations.shapes(),
            &[Annotation {
                from: 52,
                to: 36,
                brush: Brush::Green
            }]
        );
        assert!(rt.state.variations.is_empty());
    }

    #[test]
    fn modifier_combinations_match_chessground() {
        assert_eq!(Brush::from_modifiers(false, false), Brush::Green);
        assert_eq!(Brush::from_modifiers(true, false), Brush::Red);
        assert_eq!(Brush::from_modifiers(false, true), Brush::Blue);
        assert_eq!(Brush::from_modifiers(true, true), Brush::Yellow);
    }
}
