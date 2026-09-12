//! The popup: header, virtualised list, footer.
//!
//! Layout rule: each band (header, row, footer) is a fixed-height `container`
//! that **centres** its content vertically through `center_y`. A
//! `row.align_y(Center)` is not enough: it aligns children relative to each
//! other but leaves the row stuck to the top of its container.

use iced::widget::{
    canvas, column, container, mouse_area, row, scrollable, text, text_input, Space,
};
use iced::{mouse, window, Background, Border, Element, Length, Padding, Point, Rectangle};

use copycopy_core::ClipItem;

use crate::theme as t;
use crate::{Message, State, SCROLL_ID, SEARCH_ID};

pub fn view(state: &State, _window: iced::window::Id) -> Element<'_, Message> {
    let content = column![
        header(state),
        hairline(),
        list(state),
        hairline(),
        footer(state),
    ];

    // The card is on the outside and the handles inside, which keeps the
    // window opaque all the way to the edge with no transparent margin.
    container(resize_frame(content.into()))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(t::card)
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

fn hairline<'a>() -> Element<'a, Message> {
    container(Space::new().height(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(t::separator)
        .into()
}

// ----------------------------------------------------------------- header

/// Hand-drawn magnifier: independent of which glyphs the font happens to have,
/// and crisp at every scale.
struct Magnifier;

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
                .with_color(t::FAINT)
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

fn header(state: &State) -> Element<'_, Message> {
    // No `on_submit`: Enter goes through the global keyboard listener, or it
    // would be handled twice.
    let field = text_input("Rechercher dans le presse-papier…", &state.query)
        .id(SEARCH_ID)
        .on_input(Message::Query)
        .size(15.5)
        .padding(0)
        .style(|_theme, _status| text_input::Style {
            background: Background::Color(iced::Color::TRANSPARENT),
            border: Border::default(),
            icon: t::FAINT,
            placeholder: t::FAINT,
            value: t::TEXT,
            selection: t::alpha(t::ACCENT, 0.35),
        });

    // `mouse_area` lets the child capture first, so clicking the search field
    // will not start dragging the window.
    mouse_area(
        container(
            row![
                canvas(Magnifier)
                    .width(Length::Fixed(16.0))
                    .height(Length::Fixed(16.0)),
                Space::new().width(Length::Fixed(12.0)),
                field,
                Space::new().width(Length::Fixed(20.0)),
                text("↑↓ naviguer   ·   Enter copier   ·   Ctrl-B épingler   ·   Esc")
                    .size(11.0)
                    .color(t::FAINT),
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
fn list(state: &State) -> Element<'_, Message> {
    let total = state.filtered.len();
    if total == 0 {
        let msg = if state.history.is_empty() {
            "Rien de capturé pour l'instant — copiez quelque chose"
        } else {
            "Aucun résultat"
        };
        return container(text(msg).size(14.0).color(t::FAINT))
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
    let mut rows = column![];
    if skip > 0 {
        rows = rows.push(Space::new().height(Length::Fixed(skip as f32 * t::ROW_H)));
    }
    for index in skip..skip + take {
        let Some(item) = state.history.get(state.filtered[index]) else {
            continue;
        };
        rows = rows.push(row_widget(
            item,
            index,
            index == state.selected,
            state.hovered == Some(index),
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
        .style(|theme, status| scrollable::Style {
            vertical_rail: scrollable::Rail {
                background: None,
                border: Border::default(),
                scroller: scrollable::Scroller {
                    background: Background::Color(t::alpha(t::TEXT, 0.13)),
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
    selected: bool,
    hovered: bool,
) -> Element<'_, Message> {
    let tint = t::tint(item.kind);

    // Always present, transparent when the row is not selected, so the content
    // width does not shift from one row to the next.
    let accent = container(Space::new())
        .width(Length::Fixed(3.0))
        .height(Length::Fixed(24.0))
        .style(move |_| container::Style {
            background: selected.then(|| Background::Color(t::ACCENT)),
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

    let body = column![
        text(item.preview.as_str())
            .size(14.0)
            .color(t::TEXT)
            .wrapping(text::Wrapping::None),
        text(format!("{}  ·  {}", source, item.age()))
            .size(11.0)
            .color(t::FAINT)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(2)
    .width(Length::Fill);

    let mut content = row![
        accent,
        Space::new().width(Length::Fixed(9.0)),
        badge,
        Space::new().width(Length::Fixed(13.0)),
        body,
    ]
    .align_y(iced::Alignment::Center);

    if item.pinned {
        content = content.push(text("◆").size(10).color(t::ACCENT));
        content = content.push(Space::new().width(Length::Fixed(8.0)));
    }

    // No end-of-line fade: the `stack` overlay that produced it stopped the
    // row from repainting — the text stayed frozen on its first render, then
    // vanished. Previews are therefore cut off flat at the edge, at a constant
    // x, which reads as a column boundary.
    let ground = if selected {
        t::SELECTED
    } else if hovered {
        t::HOVER
    } else {
        t::CARD
    };
    let background = (ground != t::CARD).then(|| Background::Color(ground));

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

    mouse_area(styled)
        .on_press(Message::Select(index))
        .on_double_click(Message::Activate)
        .on_enter(Message::Hover(Some(index)))
        .on_exit(Message::Hover(None))
        .into()
}

// ----------------------------------------------------------------- footer

fn footer(state: &State) -> Element<'_, Message> {
    let left = match &state.flash {
        Some((msg, at)) if at.elapsed().as_secs_f32() < 3.0 => msg.clone(),
        _ => format!(
            "{} éléments   ·   {}",
            state.history.len(),
            state
                .backend
                .map(|b| b.label())
                .unwrap_or("capture indisponible")
        ),
    };

    container(
        row![
            text(left).size(11.0).color(t::DIM),
            Space::new().width(Length::Fill),
            text(state.fonts_note.as_str()).size(11.0).color(t::FAINT),
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .center_y(Length::Fixed(t::FOOTER_H))
    .padding(Padding::from([0, 18]))
    .into()
}
