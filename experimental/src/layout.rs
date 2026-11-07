//! Responsive sizing from the actual shell content area, including sidebar resizing.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub width: f32,
    pub height: f32,
}
impl Layout {
    pub fn new([width, height]: [f32; 2]) -> Self {
        Self { width, height }
    }
    pub fn padding(self) -> f32 {
        if self.width < 620.0 {
            16.0
        } else if self.width < 1000.0 {
            24.0
        } else {
            40.0
        }
    }
    pub fn stacked_review(self) -> bool {
        self.width < 760.0
    }
    pub fn inspector_width(self) -> f32 {
        if self.width >= 1160.0 {
            392.0
        } else if self.width >= 920.0 {
            352.0
        } else {
            304.0
        }
    }
    pub fn focus_board_size(self) -> f32 {
        ((self.width - 68.0)
            .min(self.height - 324.0)
            .clamp(240.0, 720.0)
            / 8.0)
            .floor()
            * 8.0
    }
    pub fn board_size(self) -> f32 {
        let available_width = if self.stacked_review() {
            self.width - 68.0
        } else {
            self.width - self.inspector_width() - 68.0
        };
        let available_height = self.height - 324.0;
        let side = available_width.min(available_height).clamp(240.0, 640.0);
        // Eight whole-point squares keep the board and glyphs on the pixel grid.
        (side / 8.0).floor() * 8.0
    }
}
