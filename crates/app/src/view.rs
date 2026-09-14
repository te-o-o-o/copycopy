//! The popup: header, virtualised list, footer.
//!
//! Layout rule: each band (header, row, footer) is a fixed-height `container`
//! that **centres** its content vertically through `center_y`. A
//! `row.align_y(Center)` is not enough: it aligns children relative to each
//! other but leaves the row stuck to the top of its container.

use iced::widget::{
    canvas, column, container, image, mouse_area, rich_text, row, scrollable, span, stack, text,
    text_input,
    Space,
};
use iced::{
    font, mouse, window, Background, Border, Element, Font, Length, Padding, Point, Rectangle,
    ContentFit, Size,
};

use copycopy_core::ClipItem;

use crate::theme::{self as t, Palette};
use crate::{Message, Preview, State, PREVIEW_ID, SCROLL_ID, SEARCH_ID};

pub fn view(state: &State, _window: iced::window::Id) -> Element<'_, Message> {
    // Read once per frame and handed down: every function below draws with
    // it, and a theme switch is nothing more than the next frame using the
    // other one.
    let p = state.palette();
    let content = column![
        header(state, p),
        hairline(p),
        filters(state, p),
        hairline(p),
        body(state, p),
        hairline(p),
        footer(state, p),
    ];

    // The rain is the one `stack` in the application, and it only exists in the
    // Matrix theme. It sits *under* the interface, never over it: a layer laid
    // over a row stops that row from repainting (rule 3), and whether a layer
    // underneath is safe is exactly what this is trying out. The other themes
    // keep the layout-only tree, untouched.
    let inside: Element<'_, Message> = if p.matrix {
        stack![
            canvas(Rain {
                t: state.rain_t,
                ink: p.text,
            })
            .width(Length::Fill)
            .height(Length::Fill),
            resize_frame(content.into()),
        ]
        .into()
    } else {
        resize_frame(content.into())
    };

    // The card is on the outside and the handles inside, which keeps the
    // window opaque all the way to the edge with no transparent margin.
    container(inside)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(t::card(p))
        .into()
}

/// The window has no decorations: these eight bands, invisible because they
/// let the card background through, provide the resize handles. They are placed
/// **by layout**, not as an overlay: a `stack` on top stopped rows from
/// repainting.
fn resize_frame(content: Element<'_, Message>) -> Element<'_, Message> {
    use window::Direction::*;

    let edge = Length::Fixed(t::EDGE);
    column![
        row![
            handle(NorthWest, edge, edge),
            handle(North, Length::Fill, edge),
            handle(NorthEast, edge, edge),
        ]
        .height(edge),
        row![
            handle(West, edge, Length::Fill),
            container(content).width(Length::Fill).height(Length::Fill),
            handle(East, edge, Length::Fill),
        ]
        .height(Length::Fill),
        row![
            handle(SouthWest, edge, edge),
            handle(South, Length::Fill, edge),
            handle(SouthEast, edge, edge),
        ]
        .height(edge),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn handle<'a>(
    direction: window::Direction,
    width: Length,
    height: Length,
) -> Element<'a, Message> {
    use window::Direction::*;
    let cursor = match direction {
        North | South => mouse::Interaction::ResizingVertically,
        East | West => mouse::Interaction::ResizingHorizontally,
        NorthWest | SouthEast => mouse::Interaction::ResizingDiagonallyDown,
        NorthEast | SouthWest => mouse::Interaction::ResizingDiagonallyUp,
    };
    mouse_area(container(Space::new()).width(width).height(height))
        .interaction(cursor)
        .on_press(Message::ResizeFrom(direction))
        .into()
}

fn hairline<'a>(p: Palette) -> Element<'a, Message> {
    container(Space::new().height(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(t::separator(p))
        .into()
}

// ----------------------------------------------------------------- header

/// Hand-drawn magnifier: independent of which glyphs the font happens to have,
/// and crisp at every scale.
struct Magnifier {
    color: iced::Color,
}

impl canvas::Program<Message> for Magnifier {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let stroke = || {
            canvas::Stroke::default()
                .with_color(self.color)
                .with_width(1.5)
        };
        frame.stroke(&canvas::Path::circle(Point::new(6.5, 6.5), 5.0), stroke());
        frame.stroke(
            &canvas::Path::line(Point::new(10.4, 10.4), Point::new(14.2, 14.2)),
            stroke(),
        );
        vec![frame.into_geometry()]
    }
}

/// Pin marker, drawn for the same reason as the magnifier: full control of
/// size and colour, and no dependence on which glyphs a font happens to ship.
struct Pin {
    color: iced::Color,
    /// The card colour, to punch the hole in the head. Passed in rather than
    /// read from a constant: it changes with the theme.
    hole: iced::Color,
}

impl canvas::Program<Message> for Pin {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        // Teardrop silhouette: a round head closed by a point underneath. It
        // reads as a pin at 16 px, where a head on a straight needle reads as
        // a balloon.
        let head = Point::new(8.0, 6.4);
        frame.fill(&canvas::Path::circle(head, 5.0), self.color);
        frame.fill(
            &canvas::Path::new(|b| {
                b.move_to(Point::new(3.6, 9.0));
                b.line_to(Point::new(12.4, 9.0));
                b.line_to(Point::new(8.0, 17.0));
                b.close();
            }),
            self.color,
        );
        // Hollow centre, so the shape stays legible against a light row.
        frame.fill(&canvas::Path::circle(head, 1.9), self.hole);
        vec![frame.into_geometry()]
    }
}

/// Matrix rain: columns of glyphs falling slowly and fading along their trail.
///
/// Stateless. Each column takes its speed, length and phase from a hash of its
/// index, and every position follows from the time alone — there is no list of
/// drops to keep up to date, and a frame draws a few hundred glyphs at most.
struct Rain {
    t: f32,
    ink: iced::Color,
}

/// Half-width katakana and a few digits and signs, as in the film.
const RAIN_GLYPHS: &str = "ｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉﾊﾋﾌﾍﾎﾏﾐﾑﾒﾓﾔﾕﾖﾗﾘﾙﾚﾛﾜﾝ0123456789:.=*+-<>";
/// Horizontal pitch of the columns, and vertical pitch of the glyphs. Twenty
/// pixels rather than the first try's twenty-two: a column spends much of its
/// cycle off screen between two trails, so extra rain only shows as extra
/// columns, not as a higher share of wet ones.
const RAIN_COLUMN: f32 = 20.0;
const RAIN_ROW: f32 = 17.0;

/// SplitMix64's finaliser: a cheap, well-spread hash, so neighbouring columns
/// get unrelated speeds without pulling a random number crate in.
fn scramble(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

impl canvas::Program<Message> for Rain {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let glyphs: Vec<char> = RAIN_GLYPHS.chars().collect();
        let columns = (bounds.width / RAIN_COLUMN) as u64 + 1;
        let rows = (bounds.height / RAIN_ROW) as i64 + 1;

        for column in 0..columns {
            let seed = scramble(column);
            // One column in three stays dry: enough rain to fill the window,
            // sparse enough to read as atmosphere rather than noise laid over
            // the list. One in two was tried first and felt too thin.
            if seed.is_multiple_of(3) {
                continue;
            }
            let speed = 1.0 + ((seed >> 8) % 200) as f32 / 100.0; // 1 to 3 rows a second
            let length = 6 + ((seed >> 20) % 10) as i64; // 6 to 15 glyphs
            let cycle = rows + length + 8 + ((seed >> 32) % 24) as i64;
            let offset = ((seed >> 40) % 1000) as f32;
            let head = ((self.t * speed + offset) % cycle as f32).floor() as i64;
            let x = column as f32 * RAIN_COLUMN + 4.0;

            for k in 0..length {
                let row = head - k;
                if !(0..rows).contains(&row) {
                    continue;
                }
                let alpha = if k == 0 {
                    0.42
                } else {
                    0.24 * (1.0 - k as f32 / length as f32)
                };
                // Each cell changes glyph about every two seconds, each on its
                // own beat: a slow flicker rather than the whole screen pulsing.
                let cell = seed ^ (row as u64).wrapping_mul(0x9E37_79B9);
                let beat = (self.t * 0.5 + (cell % 100) as f32 / 50.0) as u64;
                let pick = scramble(cell ^ beat);
                frame.fill_text(canvas::Text {
                    content: glyphs[(pick % glyphs.len() as u64) as usize].to_string(),
                    position: Point::new(x, row as f32 * RAIN_ROW),
                    color: t::alpha(self.ink, alpha),
                    size: iced::Pixels(13.0),
                    font: Font::MONOSPACE,
                    shaping: text::Shaping::Advanced,
                    ..canvas::Text::default()
                });
            }
        }
        vec![frame.into_geometry()]
    }
}

/// The application's mark: "cc" on a rounded tile, magenta fading to violet.
///
/// Drawn rather than typeset, like every other glyph here: a bold monospace
/// face is not guaranteed on every system, and the same geometry is meant to
/// become the installers' icon. Brand colours, not palette ones — a logo does
/// not change with the theme.
struct LogoMark;

impl canvas::Program<Message> for LogoMark {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let side = bounds.width.min(bounds.height);

        let tile = canvas::Path::rounded_rectangle(
            Point::ORIGIN,
            Size::new(side, side),
            (side * 0.27).into(),
        );
        let fill = canvas::gradient::Linear::new(Point::ORIGIN, Point::new(side, side))
            .add_stop(0.0, iced::Color::from_rgb8(0xF0, 0x00, 0xA0))
            .add_stop(1.0, iced::Color::from_rgb8(0x8A, 0x2B, 0xC2));
        frame.fill(&tile, fill);

        // Two arcs open on the right: each "c" runs clockwise from 45° below
        // the horizontal, round the left, to 45° above it.
        let ink = canvas::Stroke::default()
            .with_color(iced::Color::WHITE)
            .with_width(side * 0.095)
            .with_line_cap(canvas::LineCap::Round);
        for centre_x in [0.33, 0.66] {
            let c = canvas::Path::new(|b| {
                b.arc(canvas::path::Arc {
                    center: Point::new(side * centre_x, side * 0.5),
                    radius: side * 0.15,
                    start_angle: iced::Radians(std::f32::consts::FRAC_PI_4),
                    end_angle: iced::Radians(7.0 * std::f32::consts::FRAC_PI_4),
                });
            });
            frame.stroke(&c, ink);
        }
        vec![frame.into_geometry()]
    }
}

/// Settings button: a gear, which says "configuration" where three dots only say
/// "more". Eight square teeth around a hollow hub, drawn as one polygon.
struct GearMark {
    color: iced::Color,
    /// The header background, to punch the hub out of the wheel.
    hole: iced::Color,
}

impl canvas::Program<Message> for GearMark {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        const TEETH: usize = 8;
        const OUTER: f32 = 8.4;
        const INNER: f32 = 6.2;
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let centre = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        // Four corners per tooth: two on the outer radius, two on the inner.
        // The half-step offset keeps a tooth centred on each axis.
        let steps = TEETH * 4;
        let step = std::f32::consts::TAU / steps as f32;
        let wheel = canvas::Path::new(|b| {
            for i in 0..steps {
                let angle = (i as f32 - 0.5) * step;
                let radius = if i % 4 < 2 { OUTER } else { INNER };
                let point = Point::new(
                    centre.x + radius * angle.cos(),
                    centre.y + radius * angle.sin(),
                );
                if i == 0 {
                    b.move_to(point);
                } else {
                    b.line_to(point);
                }
            }
            b.close();
        });
        frame.fill(&wheel, self.color);
        frame.fill(&canvas::Path::circle(centre, 2.6), self.hole);
        vec![frame.into_geometry()]
    }
}

/// The header's close button. Larger and bolder than the row cross, which has to
/// stay discreet next to every entry, and with rounded ends.
struct CloseMark {
    color: iced::Color,
}

impl canvas::Program<Message> for CloseMark {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let stroke = || {
            canvas::Stroke::default()
                .with_color(self.color)
                .with_width(2.0)
                .with_line_cap(canvas::LineCap::Round)
        };
        let (a, b) = (5.0, 15.0);
        frame.stroke(&canvas::Path::line(Point::new(a, a), Point::new(b, b)), stroke());
        frame.stroke(&canvas::Path::line(Point::new(b, a), Point::new(a, b)), stroke());
        vec![frame.into_geometry()]
    }
}

/// Theme switch. The glyph names the theme in use — a moon while dark, a sun
/// while light — which is what most applications have taught people to read.
struct ThemeMark {
    light: bool,
    ink: iced::Color,
    /// The header background, to carve the crescent out of a full disc.
    ground: iced::Color,
}

impl canvas::Program<Message> for ThemeMark {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let centre = Point::new(8.0, 8.0);
        if self.light {
            frame.fill(&canvas::Path::circle(centre, 3.0), self.ink);
            let ray = canvas::Stroke::default().with_color(self.ink).with_width(1.4);
            for i in 0..8 {
                let angle = i as f32 * std::f32::consts::FRAC_PI_4;
                let (sin, cos) = angle.sin_cos();
                frame.stroke(
                    &canvas::Path::line(
                        Point::new(8.0 + 5.0 * cos, 8.0 + 5.0 * sin),
                        Point::new(8.0 + 7.0 * cos, 8.0 + 7.0 * sin),
                    ),
                    ray,
                );
            }
        } else {
            frame.fill(&canvas::Path::circle(centre, 5.6), self.ink);
            frame.fill(&canvas::Path::circle(Point::new(10.6, 5.8), 4.8), self.ground);
        }
        vec![frame.into_geometry()]
    }
}

/// Copy affordance: the two offset sheets everyone reads as "copy". Drawn like
/// the others, and here the reason is doubled — this one repeats on every row,
/// so a glyph whose weight shifts with the font would be visible as noise down
/// the whole list.
struct CopyMark {
    color: iced::Color,
}

impl canvas::Program<Message> for CopyMark {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let stroke = || {
            canvas::Stroke::default()
                .with_color(self.color)
                .with_width(1.3)
        };
        // The sheet behind is reduced to the corner that actually shows. A full
        // outline would draw its lines straight through the front sheet, and
        // masking them would mean painting the row background — which varies
        // with hover, selection and the copy flash, none of it known here.
        frame.stroke(
            &canvas::Path::new(|b| {
                b.move_to(Point::new(5.6, 3.4));
                b.line_to(Point::new(12.6, 3.4));
                b.line_to(Point::new(12.6, 10.4));
            }),
            stroke(),
        );
        frame.stroke(
            &canvas::Path::rectangle(Point::new(3.4, 5.6), Size::new(7.0, 7.0)),
            stroke(),
        );
        vec![frame.into_geometry()]
    }
}

/// Delete affordance. Drawn rather than a glyph: `×` and `✕` differ wildly in
/// weight from one font to the next, and this one has to stay discreet.
struct Cross {
    color: iced::Color,
}

impl canvas::Program<Message> for Cross {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let stroke = || {
            canvas::Stroke::default()
                .with_color(self.color)
                .with_width(1.6)
        };
        let (a, b) = (4.5, 11.5);
        frame.stroke(&canvas::Path::line(Point::new(a, a), Point::new(b, b)), stroke());
        frame.stroke(&canvas::Path::line(Point::new(b, a), Point::new(a, b)), stroke());
        vec![frame.into_geometry()]
    }
}

fn header(state: &State, p: Palette) -> Element<'_, Message> {
    // No `on_submit`: Enter goes through the global keyboard listener, or it
    // would be handled twice.
    let field = text_input("Rechercher dans le presse-papier…", &state.query)
        .id(SEARCH_ID)
        .on_input(Message::Query)
        .size(15.5)
        .padding(0)
        .style(move |_theme, _status| text_input::Style {
            background: Background::Color(iced::Color::TRANSPARENT),
            border: Border::default(),
            icon: p.faint,
            placeholder: p.faint,
            value: p.text,
            selection: t::alpha(p.accent, 0.35),
        });

    // `mouse_area` lets the child capture first, so clicking the search field
    // will not start dragging the window.
    mouse_area(
        container(
            row![
                canvas(LogoMark)
                    .width(Length::Fixed(22.0))
                    .height(Length::Fixed(22.0)),
                Space::new().width(Length::Fixed(14.0)),
                canvas(Magnifier { color: p.faint })
                    .width(Length::Fixed(16.0))
                    .height(Length::Fixed(16.0)),
                Space::new().width(Length::Fixed(12.0)),
                field,
                Space::new().width(Length::Fixed(20.0)),
                // The arrows need no caption; deleting does, since nothing
                // else in the window hints that it is possible.
                text("Enter copier   ·   Ctrl-B épingler   ·   Suppr supprimer   ·   Esc")
                    .size(11.0)
                    .color(p.chrome),
                Space::new().width(Length::Fixed(16.0)),
                mouse_area(
                    canvas(ThemeMark {
                        light: p.light,
                        ink: t::alpha(p.text, 0.55),
                        ground: p.card,
                    })
                    .width(Length::Fixed(t::SLOT))
                    .height(Length::Fixed(t::SLOT)),
                )
                .interaction(mouse::Interaction::Pointer)
                .on_press(Message::ToggleTheme),
                Space::new().width(Length::Fixed(12.0)),
                mouse_area(
                    canvas(GearMark {
                        color: if state.settings_open {
                            p.accent
                        } else {
                            t::alpha(p.text, 0.55)
                        },
                        hole: p.card,
                    })
                    .width(Length::Fixed(18.0))
                    .height(Length::Fixed(18.0)),
                )
                .interaction(mouse::Interaction::Pointer)
                .on_press(Message::ToggleSettings),
                Space::new().width(Length::Fixed(8.0)),
                // Closes the window, never the resident: quitting lives in the
                // settings, under its own name, so nobody stops capture by
                // reaching for the usual corner.
                mouse_area(
                    canvas(CloseMark {
                        color: t::alpha(p.text, 0.7),
                    })
                    .width(Length::Fixed(20.0))
                    .height(Length::Fixed(20.0)),
                )
                .interaction(mouse::Interaction::Pointer)
                .on_press(Message::Close),
            ]
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .center_y(Length::Fixed(t::HEADER_H))
        .padding(Padding::from([0, 18])),
    )
    .on_press(Message::DragWindow)
    .into()
}

// ------------------------------------------------------------------- list

/// Manual virtualisation: iced does not do it, but with fixed-height rows we
/// know exactly which ones are visible. That is what holds 100,000 entries at
/// 59 fps instead of collapsing from 5,000 onwards.
fn list(state: &State, p: Palette) -> Element<'_, Message> {
    let total = state.visible.len();
    if total == 0 {
        let msg = if state.history.is_empty() {
            "Rien de capturé pour l'instant — copiez quelque chose"
        } else if state.kind_filter.is_some() && state.query.trim().is_empty() {
            "Aucune entrée de ce type"
        } else {
            "Aucun résultat"
        };
        return container(text(msg).size(14.0).color(p.faint))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
    }

    const MARGIN: usize = 3;
    let first = (state.scroll_y / t::ROW_H).floor().max(0.0) as usize;
    let visible = (state.viewport_h / t::ROW_H).ceil() as usize + MARGIN * 2;
    let skip = first.saturating_sub(MARGIN);
    let take = visible.min(total.saturating_sub(skip));
    let after = total.saturating_sub(skip + take);

    // Neither `spacing` nor vertical margin: the pitch from one row to the
    // next must be exactly ROW_H, otherwise the virtualisation spacers drift
    // away from the real scroll position.
    let position = state
        .selection
        .interpolate_with(|v| v, std::time::Instant::now());
    let mut rows = column![];
    if skip > 0 {
        rows = rows.push(Space::new().height(Length::Fixed(skip as f32 * t::ROW_H)));
    }
    for index in skip..skip + take {
        let Some(item) = state.visible.get(index) else {
            continue;
        };
        // Distance to the animated selection: 1 on the incoming row, 0 once
        // it has moved away, and a blend in between.
        let weight = (1.0 - (index as f32 - position).abs()).clamp(0.0, 1.0);
        rows = rows.push(row_widget(
            item,
            index,
            weight,
            state.hovered == Some(index),
            state.copied.is_some_and(|c| c.id == item.id),
            p,
        ));
    }
    if after > 0 {
        rows = rows.push(Space::new().height(Length::Fixed(after as f32 * t::ROW_H)));
    }

    // Left margin only: on the right the scrollbar reserves its own through
    // `spacing`, otherwise it ends up touching the highlight. The left value is
    // chosen so both margins of the highlight come out equal, with the
    // scrollbar floating inside the right one.
    scrollable(rows.padding(Padding {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 14.0,
    }))
        .id(SCROLL_ID)
        .height(Length::Fill)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(5)
                .scroller_width(5)
                .margin(3)
                .spacing(4),
        ))
        .style(move |theme, status| scrollable::Style {
            vertical_rail: scrollable::Rail {
                background: None,
                border: Border::default(),
                scroller: scrollable::Scroller {
                    background: Background::Color(t::alpha(p.text, 0.13)),
                    border: Border::default().rounded(3),
                },
            },
            ..scrollable::default(theme, status)
        })
        .on_scroll(Message::Scrolled)
        .into()
}

fn row_widget(
    item: &ClipItem,
    index: usize,
    selected: f32,
    hovered: bool,
    copied: bool,
    p: Palette,
) -> Element<'_, Message> {
    let tint = p.tint(item.kind);

    // Always present, transparent when the row is not selected, so the content
    // width does not shift from one row to the next.
    let accent = container(Space::new())
        .width(Length::Fixed(3.0))
        .height(Length::Fixed(24.0))
        .style(move |_| container::Style {
            background: if copied {
                Some(Background::Color(p.copied))
            } else if selected > 0.0 {
                Some(Background::Color(t::alpha(p.accent, selected)))
            } else {
                None
            },
            border: Border::default().rounded(2),
            ..Default::default()
        });

    let badge = container(
        text(t::badge(item.kind))
            .size(9.5)
            .color(tint)
            .font(iced::Font::MONOSPACE),
    )
    .center(Length::Fixed(30.0))
    .style(move |_| container::Style {
        background: Some(Background::Color(t::alpha(tint, 0.16))),
        border: Border::default().rounded(9),
        ..Default::default()
    });

    let source = if item.source.is_empty() {
        "—"
    } else {
        item.source.as_str()
    };

    // Right-hand gutter: the pin slot, then the cross slot. Both are always
    // laid out; only what they contain is conditional, so the preview always
    // clips at the same x.
    let pin_slot: Element<'_, Message> = if item.pinned {
        canvas(Pin {
            color: p.pin,
            hole: p.card,
        })
            .width(Length::Fixed(t::SLOT))
            .height(Length::Fixed(t::SLOT))
            .into()
    } else {
        Space::new().width(Length::Fixed(t::SLOT)).into()
    };

    // Shown on the row under the pointer, and on the selected row, so one is
    // always visible without repeating a cross on every line.
    let cross_slot: Element<'_, Message> = if hovered || selected > 0.5 {
        mouse_area(
            canvas(Cross {
                color: if hovered { p.text } else { t::alpha(p.text, 0.45) },
            })
            .width(Length::Fixed(t::SLOT))
            .height(Length::Fixed(t::SLOT)),
        )
        .interaction(mouse::Interaction::Pointer)
        .on_press(Message::Delete(index))
        .into()
    } else {
        Space::new().width(Length::Fixed(t::SLOT)).into()
    };

    // Always drawn, unlike the cross: copying is the point of the application,
    // so the affordance does not wait to be discovered by hovering. It still
    // brightens with the row, to say which one it would act on.
    let copy_slot: Element<'_, Message> = mouse_area(
        canvas(CopyMark {
            color: if hovered { p.text } else { t::alpha(p.text, 0.30) },
        })
        .width(Length::Fixed(t::SLOT))
        .height(Length::Fixed(t::SLOT)),
    )
    .interaction(mouse::Interaction::Pointer)
    .on_press(Message::CopyRow(index))
    .into();

    let preview = container(
        text(item.preview.as_str())
            .size(14.0)
            .font(Font {
                weight: font::Weight::Semibold,
                ..Font::DEFAULT
            })
            .color(p.text)
            .wrapping(text::Wrapping::None),
    )
    .width(Length::Fill)
    // Clipped at its own width, not the row's. `Wrapping::None` gives a text
    // widget the intrinsic width of its whole string, so without this the
    // preview draws straight over the gutter — the row's own clip only stops it
    // at the far edge.
    .clip(true);

    // The meta line recedes through opacity rather than a flat grey: it keeps
    // the same hue as the preview, so the two read as one block at two depths.
    let meta = container(
        text(match item.overflow_hint() {
            Some(hint) => format!("{}  ·  {}  ·  {}", source, item.age(), hint),
            None => format!("{}  ·  {}", source, item.age()),
        })
            .size(11.0)
            .color(t::alpha(p.text, 0.42))
            .wrapping(text::Wrapping::None),
    )
    .width(Length::Fill)
    .clip(true);

    // The gutter sits on the preview line, not on the row: centring it over the
    // whole row would drag it down by half the meta line, leaving pin and cross
    // visibly below the text they belong to.
    let top = row![
        preview,
        Space::new().width(Length::Fixed(t::GUTTER_GAP)),
        copy_slot,
        Space::new().width(Length::Fixed(8.0)),
        cross_slot,
    ]
    .align_y(iced::Alignment::Center);

    let body = column![top, meta].spacing(2);

    // Ranked, and the order is the whole point. The row being copied is almost
    // always the selected one — you press Enter on it — so with the selection
    // first the green was never reached: the confirmation came down to a three
    // pixel accent bar, and a copy read as nothing happening at all.
    let background = if copied {
        Some(Background::Color(t::alpha(p.copied, 0.38)))
    } else if selected > 0.0 {
        Some(Background::Color(t::alpha(p.selected, selected)))
    } else if hovered {
        Some(Background::Color(p.hover))
    } else {
        None
    };

    // The pin sits on the left, and its slot is laid out whether or not
    // anything is drawn in it: pinning a row must not shift the badge and the
    // preview of every row around it.
    let content = row![
        accent,
        Space::new().width(Length::Fixed(9.0)),
        pin_slot,
        Space::new().width(Length::Fixed(6.0)),
        badge,
        Space::new().width(Length::Fixed(13.0)),
        body,
    ]
    .align_y(iced::Alignment::Center);

    let highlight = container(content)
        .width(Length::Fill)
        .center_y(Length::Fixed(t::ROW_H - 2.0 * t::ROW_GAP))
        .padding(Padding::from([0, 10]))
        .clip(true)
        .style(move |_| container::Style {
            background,
            border: Border::default().rounded(t::ROW_RADIUS),
            ..Default::default()
        });

    // The row keeps exactly ROW_H, which the virtualisation maths relies on.
    // Only the highlight is shrunk, which is where the spacing comes from.
    let styled = container(highlight)
        .width(Length::Fill)
        .height(Length::Fixed(t::ROW_H))
        .padding(Padding::from([t::ROW_GAP, 0.0]));

    let area = mouse_area(styled)
        .on_press(Message::Select(index))
        .on_double_click(Message::Activate)
        .on_enter(Message::Hover(index))
        .on_exit(Message::Unhover(index));

    // Only images can be dragged out (see `drag.rs`), so only they pay for
    // watching every pointer move over the row — and only they get the hand,
    // which says so before the row is even pressed.
    //
    // `Pointer`, not `Grab`: Windows has no native open/closed-hand cursor,
    // so winit falls back to `IDC_SIZEALL` for `Grab`/`Grabbing` — the
    // four-way move arrows, which reads as a cross, not a hand. `Pointer`
    // maps to `IDC_HAND`, the actual pointing hand, and is what the rest of
    // the app already uses for anything clickable.
    if item.kind == copycopy_core::Kind::Image {
        area.on_move(move |pos| Message::ImageDragMoved(index, pos))
            .on_release(Message::ImageDragReleased)
            .interaction(mouse::Interaction::Pointer)
            .into()
    } else {
        area.into()
    }
}

// ----------------------------------------------------------------- footer

/// The list and the detail panel, side by side. Proportions rather than fixed
/// widths, so resizing the window shares the room out instead of starving one
/// side; the list keeps the larger share, being what you navigate.
fn body(state: &State, p: Palette) -> Element<'_, Message> {
    row![
        container(list(state, p))
            .width(Length::FillPortion(62))
            .height(Length::Fill),
        container(Space::new().width(Length::Fixed(1.0)))
            .height(Length::Fill)
            .style(t::separator(p)),
        container(panel(state, p))
            .width(Length::FillPortion(38))
            .height(Length::Fill),
    ]
    .height(Length::Fill)
    .into()
}

/// The selected entry in full. Everything here reads `state.preview`, built
/// when the selection changed: nothing is loaded, counted or cut per frame.
fn panel(state: &State, p: Palette) -> Element<'_, Message> {
    if state.settings_open {
        return settings(state, p);
    }
    let Some(item) = state.visible.get(state.selected) else {
        return Space::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
    };

    let source = if item.source.is_empty() {
        "—"
    } else {
        item.source.as_str()
    };
    let detail = match &state.preview {
        Preview::Text {
            chars, lines, lang, ..
        } => {
            let counts = format!(
                "{} car.  ·  {} {}",
                copycopy_core::grouped(*chars),
                copycopy_core::grouped(*lines),
                plural(*lines, "ligne", "lignes")
            );
            match lang {
                Some(lang) => format!("{}  ·  {counts}", lang.name()),
                None => counts,
            }
        }
        Preview::Image {
            size: Some((w, h)), ..
        } => format!("{w}×{h}"),
        Preview::Files(paths) => {
            let n = paths.len();
            format!("{n} {}", plural(n, "fichier", "fichiers"))
        }
        _ => String::new(),
    };
    let mut caption = format!("{}  ·  {}  ·  {}", t::badge(item.kind), source, item.age());
    if !detail.is_empty() {
        caption.push_str("  ·  ");
        caption.push_str(&detail);
    }
    let caption = container(
        text(caption)
            .size(11.0)
            .color(p.faint)
            .wrapping(text::Wrapping::None),
    )
    .width(Length::Fill)
    .clip(true);

    let content: Element<'_, Message> = match &state.preview {
        Preview::Text {
            body,
            code,
            cut,
            links,
            ..
        } => {
            let font = if *code { Font::MONOSPACE } else { Font::DEFAULT };
            // `WordOrGlyph`, not the default `Word`: a URL with no spaces is a
            // single word, so plain word-wrapping never breaks it — it just
            // keeps growing past the frame instead. Falling back to a glyph
            // break is what actually keeps it inside the window.
            let content: Element<'_, Message> = if links.is_empty() {
                text(body.as_str())
                    .size(13.0)
                    .font(font)
                    .color(p.text)
                    .width(Length::Fill)
                    .wrapping(text::Wrapping::WordOrGlyph)
                    .into()
            } else {
                rich_text(linked_spans(body, links, p))
                    .size(13.0)
                    .font(font)
                    .color(p.text)
                    .width(Length::Fill)
                    .wrapping(text::Wrapping::WordOrGlyph)
                    .on_link_click(Message::OpenLink)
                    .into()
            };
            let mut lines = column![content].spacing(10);
            if *cut {
                lines = lines.push(
                    text("… la suite n'est pas affichée")
                        .size(11.0)
                        .color(p.faint),
                );
            }
            framed(lines.into(), p)
        }
        // ScaleDown, never up: a small screenshot stays sharp at its real size
        // instead of being blown up into a blur.
        Preview::Image { handle, .. } => container(
            image(handle.clone()).content_fit(ContentFit::ScaleDown),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into(),
        Preview::Files(paths) => framed(
            iced::widget::Column::with_children(paths.iter().map(|path| {
                text(path.as_str())
                    .size(12.5)
                    .font(Font::MONOSPACE)
                    .color(p.text)
                    .into()
            }))
            .spacing(6)
            .into(),
            p,
        ),
        Preview::Unavailable(why) => text(why.as_str()).size(12.0).color(p.faint).into(),
        Preview::Empty => Space::new().into(),
    };

    column![caption, content]
        .spacing(12)
        .padding(Padding {
            top: 14.0,
            right: 8.0,
            bottom: 10.0,
            left: 16.0,
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// The preview body cut into plain and linked spans, from ranges found when the
/// preview was built. Links take the accent and an underline: colour alone
/// would not tell a link from a highlighted word.
fn linked_spans<'a>(
    body: &'a str,
    links: &[std::ops::Range<usize>],
    p: Palette,
) -> Vec<iced::advanced::text::Span<'a, String, Font>> {
    let mut spans = Vec::with_capacity(links.len() * 2 + 1);
    let mut at = 0;
    for link in links {
        if link.start > at {
            spans.push(span(&body[at..link.start]));
        }
        let address = &body[link.clone()];
        spans.push(
            span(address)
                .link(address.to_string())
                .underline(true)
                .color(p.accent),
        );
        at = link.end;
    }
    if at < body.len() {
        spans.push(span(&body[at..]));
    }
    spans
}

/// French agreement: singular for 0 and 1, as French counts them.
fn plural(n: usize, one: &'static str, many: &'static str) -> &'static str {
    if n > 1 {
        many
    } else {
        one
    }
}

/// The type filters, on their own band under the search. Neutral pills, the
/// same as the settings': the list below already carries the type colours in its
/// badges, and a second set would only compete with them. Counts come from
/// `refilter`, never from here.
fn filters(state: &State, p: Palette) -> Element<'_, Message> {
    let mut pills = row![].spacing(8).align_y(iced::Alignment::Center);
    for (label, kind) in crate::FILTERS {
        let count = kind.map_or(state.counts.all, |k| state.counts.of(k));
        pills = pills.push(choice_pill(
            label,
            Some(count),
            Message::SetKindFilter(kind),
            state.kind_filter == kind,
            p,
        ));
    }
    container(pills)
        .width(Length::Fill)
        .center_y(Length::Fixed(t::FILTERS_H))
        .padding(Padding::from([0, 24]))
        .into()
}

/// A rounded choice, with an optional count. One control shared by the settings
/// and the type filters, so the two always look alike. An empty choice stays in
/// place, only dimmed: pills must not shift as the content changes.
fn choice_pill<'a>(
    label: &'static str,
    count: Option<usize>,
    on_press: Message,
    active: bool,
    p: Palette,
) -> Element<'a, Message> {
    let empty = count == Some(0) && !active;
    let ink = if active {
        p.text
    } else if empty {
        t::alpha(p.faint, 0.55)
    } else {
        p.faint
    };
    let mut content = row![text(label).size(12.5).color(ink)].align_y(iced::Alignment::Center);
    if let Some(n) = count {
        content = content.push(Space::new().width(Length::Fixed(6.0)));
        content = content.push(text(n.to_string()).size(11.0).color(t::alpha(ink, 0.7)));
    }
    mouse_area(
        container(content)
            .padding(Padding::from([4, 12]))
            .style(move |_| container::Style {
                background: active.then_some(Background::Color(t::alpha(p.accent, 0.18))),
                border: Border {
                    color: if active {
                        p.accent
                    } else if empty {
                        t::alpha(p.border, 0.5)
                    } else {
                        p.border
                    },
                    width: 1.0,
                    radius: 999.0.into(),
                },
                ..Default::default()
            }),
    )
    .interaction(mouse::Interaction::Pointer)
    .on_press(on_press)
    .into()
}

/// The settings, in the panel's place. Laid out like the preview — caption, then
/// a card — so opening them reads as the panel changing page, not as something
/// laid over the window.
fn settings(state: &State, p: Palette) -> Element<'_, Message> {
    let label = |s: &'static str| text(s).size(11.0).color(p.faint);

    let pill = move |name: &'static str, on_press: Message, active: bool| {
        choice_pill(name, None, on_press, active, p)
    };
    let current = state.theme_mode();
    // Two rows: the neutral pair first, since it is the default and the one
    // most people want; the dressier options below, for anyone who came
    // looking for them.
    let themes = column![
        row![
            pill(
                "Sombre",
                Message::SetTheme(crate::theme::Mode::Dark),
                current == crate::theme::Mode::Dark,
            ),
            pill(
                "Clair",
                Message::SetTheme(crate::theme::Mode::Light),
                current == crate::theme::Mode::Light,
            ),
        ]
        .spacing(8),
        row![
            pill(
                "Purpledream",
                Message::SetTheme(crate::theme::Mode::Purpledream),
                current == crate::theme::Mode::Purpledream,
            ),
            pill(
                "Aalto",
                Message::SetTheme(crate::theme::Mode::Aalto),
                current == crate::theme::Mode::Aalto,
            ),
            pill(
                "Matrix",
                Message::SetTheme(crate::theme::Mode::Matrix),
                current == crate::theme::Mode::Matrix,
            ),
        ]
        .spacing(8),
    ]
    .spacing(6);

    let hotkey = row![
        container(text(state.hotkey()).size(12.5).font(Font::MONOSPACE).color(p.text))
            .padding(Padding::from([3, 10]))
            .style(move |_| container::Style {
                border: Border {
                    color: p.border,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            }),
        Space::new().width(Length::Fixed(10.0)),
        text("modifiable dans copycopy.conf").size(11.5).color(p.faint),
    ]
    .align_y(iced::Alignment::Center);

    // Offered only where it can work. Elsewhere the reason shows instead of a
    // switch that would do nothing.
    let paste: Element<'_, Message> = match copycopy_platform::paste::availability() {
        Ok(()) => column![
            row![
                pill("Activé", Message::SetAutoPaste(true), state.auto_paste()),
                pill("Désactivé", Message::SetAutoPaste(false), !state.auto_paste()),
            ]
            .spacing(8),
            text("colle l'entrée copiée dans l'application où vous étiez")
                .size(11.5)
                .color(p.faint),
        ]
        .spacing(6)
        .into(),
        // `WordOrGlyph` + `Fill`: this reason routinely carries a path (a
        // network location, here), which needs the same fallback to glyph
        // wrapping as `data` below, and for the same reason — see there.
        Err(reason) => text(reason)
            .size(11.5)
            .color(p.faint)
            .width(Length::Fill)
            .wrapping(text::Wrapping::WordOrGlyph)
            .into(),
    };

    // Same treatment: where the entry cannot be written — an executable living
    // on a network path, typically — the reason replaces the switch.
    let startup: Element<'_, Message> = match crate::autostart::availability() {
        Ok(()) => column![
            row![
                pill("Activé", Message::SetAutostart(true), state.autostart()),
                pill("Désactivé", Message::SetAutostart(false), !state.autostart()),
            ]
            .spacing(8),
            text("copycopy attend en fond dès l'ouverture de session")
                .size(11.5)
                .color(p.faint),
        ]
        .spacing(6)
        .into(),
        Err(reason) => text(reason)
            .size(11.5)
            .color(p.faint)
            .width(Length::Fill)
            .wrapping(text::Wrapping::WordOrGlyph)
            .into(),
    };

    // `WordOrGlyph`: a path has no spaces to word-wrap at, and on Windows it
    // routinely outgrows the panel width — without a fallback to glyph
    // breaking it just runs past the card's edge instead of onto a second
    // line.
    let data = text(
        state
            .data_dir
            .as_ref()
            .map_or_else(|| "introuvable".to_string(), |d| d.display().to_string()),
    )
    .size(11.5)
    .font(Font::MONOSPACE)
    .color(p.text)
    .width(Length::Fill)
    .wrapping(text::Wrapping::WordOrGlyph);

    let quit = mouse_area(
        container(text("Quitter copycopy").size(12.5).color(p.text))
            .padding(Padding::from([6, 14]))
            .style(move |_| container::Style {
                border: Border {
                    color: t::alpha(p.text, 0.35),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            }),
    )
    .interaction(mouse::Interaction::Pointer)
    .on_press(Message::Quit);

    let card = container(
        column![
            label("THÈME"),
            themes,
            Space::new().height(Length::Fixed(8.0)),
            label("RACCOURCI"),
            hotkey,
            Space::new().height(Length::Fixed(8.0)),
            label("COLLAGE AUTOMATIQUE"),
            paste,
            Space::new().height(Length::Fixed(8.0)),
            label("DÉMARRAGE"),
            startup,
            Space::new().height(Length::Fixed(8.0)),
            label("DONNÉES"),
            data,
            Space::new().height(Length::Fixed(14.0)),
            row![
                text(concat!("copycopy v", env!("CARGO_PKG_VERSION"), " · MIT"))
                    .size(11.0)
                    .color(p.faint),
                Space::new().width(Length::Fill),
                quit,
            ]
            .align_y(iced::Alignment::Center),
        ]
        .spacing(8),
    )
    .padding(Padding::from([18, 18]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(p.frame)),
        border: Border {
            color: t::alpha(p.border, 0.6),
            width: 1.0,
            radius: 10.0.into(),
        },
        shadow: iced::Shadow {
            color: p.shadow,
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 22.0,
        },
        ..Default::default()
    });

    column![
        text("RÉGLAGES").size(11.0).color(p.faint),
        // Scrolls rather than squeezes: once the filter band took its share of
        // the height, the card no longer fitted and its last row — the quit
        // button — was crushed to a line.
        scrollable(container(card).padding(Padding {
            top: 2.0,
            right: 12.0,
            bottom: 24.0,
            left: 0.0,
        }))
        .height(Length::Fill)
        .direction(thin_scrollbar())
        .style(quiet_scroll(p)),
    ]
    .spacing(12)
    .padding(Padding {
        top: 14.0,
        right: 8.0,
        bottom: 10.0,
        left: 16.0,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// A Carbon-style window around a text preview: rounded, lifted off the panel
/// by a shadow, a copy button in its title bar. Built through layout like
/// everything else — the button sits in a row above the text, not in a layer
/// laid over it.
fn framed<'a>(inner: Element<'a, Message>, p: Palette) -> Element<'a, Message> {
    // The title bar keeps a single control: copying what the window shows.
    // It is the selected entry, so this is the same as pressing Enter.
    let bar = row![
        Space::new().width(Length::Fill),
        mouse_area(
            canvas(CopyMark {
                color: t::alpha(p.text, 0.55),
            })
            .width(Length::Fixed(t::SLOT))
            .height(Length::Fixed(t::SLOT)),
        )
        .interaction(mouse::Interaction::Pointer)
        .on_press(Message::Activate),
    ]
    .padding(Padding {
        top: 0.0,
        right: 10.0,
        bottom: 0.0,
        left: 0.0,
    });

    let body = scrollable(container(inner).padding(Padding {
        top: 0.0,
        right: 16.0,
        bottom: 0.0,
        left: 0.0,
    }))
    .id(PREVIEW_ID)
    // Shrink, as in Carbon: a short entry gets a small window hugging it, a
    // long one grows until the panel is full and then scrolls inside.
    .height(Length::Shrink)
    .direction(thin_scrollbar())
    .style(quiet_scroll(p));

    let window = container(column![bar, body].spacing(10))
        .padding(Padding {
            top: 14.0,
            right: 6.0,
            bottom: 18.0,
            left: 18.0,
        })
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(p.frame)),
            border: Border {
                color: t::alpha(p.border, 0.6),
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: iced::Shadow {
                color: p.shadow,
                offset: iced::Vector::new(0.0, 8.0),
                blur_radius: 22.0,
            },
            ..Default::default()
        });

    // Room around the window for the shadow to fall into, mostly below.
    container(window)
        .padding(Padding {
            top: 2.0,
            right: 12.0,
            bottom: 24.0,
            left: 0.0,
        })
        .width(Length::Fill)
        .into()
}

/// The list's scrollbar, shared with the panel: thin, and only the scroller
/// drawn, so it never competes with the content.
fn thin_scrollbar() -> scrollable::Direction {
    scrollable::Direction::Vertical(
        scrollable::Scrollbar::new()
            .width(5)
            .scroller_width(5)
            .margin(3)
            .spacing(4),
    )
}

fn quiet_scroll(
    p: Palette,
) -> impl Fn(&iced::Theme, scrollable::Status) -> scrollable::Style {
    move |theme, status| scrollable::Style {
        vertical_rail: scrollable::Rail {
            background: None,
            border: Border::default(),
            scroller: scrollable::Scroller {
                background: Background::Color(t::alpha(p.text, 0.13)),
                border: Border::default().rounded(3),
            },
        },
        ..scrollable::default(theme, status)
    }
}

fn footer(state: &State, p: Palette) -> Element<'_, Message> {
    let left = match &state.flash {
        Some((msg, at)) if at.elapsed().as_secs_f32() < 3.0 => msg.clone(),
        _ => format!("{} éléments", state.history.len()),
    };

    // A wink rather than a setting: the Matrix palette is one click away, and
    // the same click brings the usual dark theme back.
    let matrix = mouse_area(
        text(if p.matrix { "exit matrix" } else { "matrix" })
            .size(11.0)
            .font(Font::MONOSPACE)
            .color(if p.matrix {
                p.accent
            } else {
                t::alpha(p.text, 0.28)
            }),
    )
    .interaction(mouse::Interaction::Pointer)
    .on_press(Message::ToggleMatrix);

    container(
        row![
            // `Fill` + `clip`, not wrapping: the footer is one fixed-height
            // line, so a long flash message (an OS error can run long)
            // clips here the same way a row's own preview does, rather than
            // pushing `matrix` off the edge or spilling onto a second line
            // the footer has no room for.
            container(
                text(left)
                    .size(11.0)
                    .color(p.chrome)
                    .wrapping(text::Wrapping::None),
            )
            .width(Length::Fill)
            .clip(true),
            matrix,
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .center_y(Length::Fixed(t::FOOTER_H))
    .padding(Padding::from([0, 18]))
    .into()
}
