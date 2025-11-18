//! Byzantine Border Demo
//! Demonstrates the three Byzantine border styles:
//! 1. ByzantineBorder (standard gold dark)
//! 2. SimpleByzantineCard (bright gold)
//! 3. PremiumByzantineCard (bright gold, opaque)

use iced::widget::{column, text};
use iced::{Application, Command, Element, Length, Settings, Theme};
use kindly_dedup::gui::widgets::{ByzantineBorder, PremiumByzantineCard, SimpleByzantineCard};

pub fn main() -> iced::Result {
    ByzantineDemo::run(Settings::default())
}

struct ByzantineDemo;

#[derive(Debug, Clone)]
enum Message {}

impl Application for ByzantineDemo {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        (Self, Command::none())
    }

    fn title(&self) -> String {
        "Byzantine Border Demo - kindly_dedup".to_string()
    }

    fn update(&mut self, _message: Message) -> Command<Message> {
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        column![
            text("Byzantine Border Variants").size(32),
            text(""),
            // Standard Byzantine border (3px gold dark)
            ByzantineBorder::new(
                column![
                    text("Standard Byzantine Border").size(24),
                    text("3px GOLD_DARK (#DAA520) border"),
                    text("16px border radius"),
                    text("75% opacity CARD_BG background"),
                ]
                .spacing(8)
            )
            .padding(24)
            .view(),
            text(""),
            // Simplified Byzantine card (2px bright gold)
            SimpleByzantineCard::new(
                column![
                    text("Simplified Byzantine Card").size(24),
                    text("2px GOLD_BRIGHT (#FFD700) border"),
                    text("18px border radius"),
                    text("80% opacity CARD_BG background"),
                ]
                .spacing(8)
            )
            .view(),
            text(""),
            // Premium Byzantine card (3px bright gold, opaque)
            PremiumByzantineCard::new(
                column![
                    text("Premium Byzantine Card").size(24),
                    text("3px GOLD_BRIGHT (#FFD700) border"),
                    text("20px border radius"),
                    text("90% opacity CARD_BG background (more opaque)"),
                ]
                .spacing(8)
            )
            .view(),
        ]
        .padding(40)
        .spacing(20)
        .width(Length::Fill)
        .into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}
