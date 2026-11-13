//! Vector chess pieces with contrasting fills and outlines.
use rune::Icon;

macro_rules! piece {
    ($body:ident, $lines:ident, $name:literal, $shape:literal, $detail:literal) => {
        pub static $body: Icon = Icon {
            name: concat!($name, "-body"),
            view: 45.0,
            body: concat!("<path d=\"", $shape, "\" fill=\"#fff\"/>"),
        };
        pub static $lines: Icon = Icon {
            name: concat!($name, "-lines"),
            view: 45.0,
            body: concat!("<path d=\"", $shape, "\" fill=\"none\" stroke=\"#fff\" stroke-width=\"1.8\" stroke-linejoin=\"round\"/>", "<path d=\"", $detail, "\" fill=\"none\" stroke=\"#fff\" stroke-width=\"1.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>"),
        };
    };
}

piece!(
    KING_BODY,
    KING_LINES,
    "king",
    "M20 3H25V7H29V11H25V14C31 10 36 14 35 20L30 30H32L34 35H36V40H9V35H11L13 30H15L10 20C9 14 14 10 20 14V11H16V7H20Z",
    "M15 20C18 18 20 19 22.5 22C25 19 27 18 30 20M14 30H31M12 35H33"
);
piece!(
    QUEEN_BODY,
    QUEEN_LINES,
    "queen",
    "M8 12L14 19L17 10L22.5 18L28 10L31 19L37 12L33 29H31L34 35H36V40H9V35H11L14 29H12Z",
    "M13 25C19 27 26 27 32 25M14 30H31M12 35H33"
);
piece!(
    ROOK_BODY,
    ROOK_LINES,
    "rook",
    "M10 7H16V11H20V7H25V11H29V7H35V16L31 20V30L34 35H36V40H9V35L14 30V20L10 16Z",
    "M11 16H34M14 20H31M14 30H31M12 35H33"
);
piece!(
    BISHOP_BODY,
    BISHOP_LINES,
    "bishop",
    "M22.5 4C25 8 30 11 30 16C30 19 28 22 26 24L30 30H32L34 35H36V40H9V35H11L13 30H15L19 24C17 22 15 19 15 16C15 11 20 8 22.5 4Z",
    "M22 12L26 16M16 24H29M14 30H31M12 35H33"
);
piece!(
    KNIGHT_BODY,
    KNIGHT_LINES,
    "knight",
    "M12 35H34V40H9V35H11L15 30C15 25 18 22 20 20L15 22L11 20L12 16L18 12L21 6L25 10C32 11 35 17 34 25L32 34Z",
    "M22 13L23 13M15 18L19 16M20 29C22 25 25 23 29 22M13 35H33"
);
piece!(
    PAWN_BODY,
    PAWN_LINES,
    "pawn",
    "M22.5 7C26 7 29 10 29 13.5C29 16 28 18 26 19C29 21 30 25 30 29L33 34H35V40H10V34H12L15 29C15 25 16 21 19 19C17 18 16 16 16 13.5C16 10 19 7 22.5 7Z",
    "M17 22C20 23 25 23 28 22M15 30H30M12 35H33"
);
