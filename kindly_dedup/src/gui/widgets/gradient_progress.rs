//! Custom gradient progress bar widget (Purple → Gold)
//! Simplified for iced 0.10

use crate::gui::theme::colors::*;
use iced::widget::progress_bar;
use iced::{Element, Length, Theme};

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

        progress_bar(0.0..=1.0, progress)
            .height(Length::Fixed(24.0))
            .style(iced::theme::ProgressBar::Custom(Box::new(GradientProgressStyle {
                progress,
            })))
            .into()
    }
}

struct GradientProgressStyle {
    progress: f32,
}

impl progress_bar::StyleSheet for GradientProgressStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> progress_bar::Appearance {
        let color = lerp_color(PURPLE_ROYAL, GOLD_BRIGHT, self.progress);
        progress_bar::Appearance {
            background: iced::Background::Color(with_alpha(PURPLE_DEEP, 0.3)),
            bar: iced::Background::Color(color),
            border_radius: 6.0.into(),
        }
    }
}
