//! Custom gradient progress bar widget (Purple → Gold)
//! Simplified for iced 0.13 with closure-based styling

use crate::gui::theme::colors::*;
use iced::widget::{container, progress_bar};
use iced::{Border, Element, Length};

/// Purple → Gold gradient progress bar
pub struct GradientProgress {
    progress: f32,
}

impl GradientProgress {
    pub fn new(progress: f32) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
        }
    }

    pub fn view(&self) -> Element<'static, super::super::messages::Message> {
        // Use progress_bar with gradient color
        let progress = self.progress;

        container(
            progress_bar(0.0..=1.0, progress)
                .style(move |_theme| {
                    let color = lerp_color(PURPLE_ROYAL, GOLD_BRIGHT, progress);
                    progress_bar::Style {
                        background: iced::Background::Color(with_alpha(PURPLE_DEEP, 0.3)),
                        bar: iced::Background::Color(color),
                        border: Border::default().rounded(6),
                    }
                })
        )
        .height(24.0)
        .into()
    }
}
