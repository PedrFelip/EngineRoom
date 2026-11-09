//! Responsive sizing from the actual shell content area, including sidebar resizing.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub width: f32,
    pub height: f32,
    inspector_width: f32,
}
impl Layout {
    pub fn new([width, height]: [f32; 2]) -> Self {
        Self {
            width,
            height,
            inspector_width: 352.0,
        }
    }
    pub fn with_inspector_width(mut self, width: f32) -> Self {
        self.inspector_width = width;
        self
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
        self.width < self.inspector_width() + 320.0 + 78.0
    }
    pub fn compact_height(self) -> bool {
        self.height < 640.0
    }
    pub fn header_height(self) -> f32 {
        if self.compact_height() { 48.0 } else { 56.0 }
    }
    pub fn inspector_width(self) -> f32 {
        self.inspector_width
    }
    fn board_for(self, lateral: bool, toolbar: bool) -> f32 {
        let width = self.width
            - 68.0
            - if lateral {
                self.inspector_width() + 10.0
            } else {
                0.0
            };
        // These are the rendered player rows, coordinates, playback, gaps, and padding.
        let workspace = if self.compact_height() { 146.0 } else { 168.0 };
        let chrome = self.header_height() + if toolbar { 36.0 } else { 0.0 } + 1.0;
        (width
            .min(self.height - chrome - workspace)
            .clamp(240.0, 800.0)
            / 8.0)
            .floor()
            * 8.0
    }
    pub fn focus_board_size(self) -> f32 {
        self.board_for(false, true)
    }
    pub fn board_size(self) -> f32 {
        self.board_for(!self.stacked_review(), self.stacked_review())
    }
}
