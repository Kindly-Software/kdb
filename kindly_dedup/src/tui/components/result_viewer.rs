//! Result Viewer Component - Tables and Charts for Analysis Results
//!
//! # UCE34 Framework
//! - Q1-Q9: Display deduplication results with formatted tables and charts
//! - Q10: N/A (read-only display, no capsule state needed)
//! - Q11-Q21: Ratatui rendering, scrollable tables, chart visualization
//! - Q31: Simplicity - Clean API for result display
//! - Q33: Validation N/A (display-only component)
//! - Q34: Auditability N/A (read-only viewer)
//!
//! # Features
//! - Formatted table rendering (cluster summary)
//! - Bar chart visualization (cluster size distribution)
//! - Scrollable text viewer (detailed results)
//! - Export confirmation dialog

use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Bar, BarChart, BarGroup, Block, Borders, Cell, List, ListItem, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table,
    },
    Frame,
};
use std::collections::HashMap;

/// Deduplication results
#[derive(Debug, Clone)]
pub struct DedupResults {
    /// Total documents processed
    pub total_docs: usize,

    /// Number of duplicate clusters found
    pub num_clusters: usize,

    /// Total duplicates found
    pub total_duplicates: usize,

    /// Cluster size distribution (size -> count)
    pub cluster_distribution: HashMap<usize, usize>,

    /// Sample clusters (for display)
    pub sample_clusters: Vec<ClusterSample>,

    /// Processing time in seconds
    pub elapsed_seconds: f64,

    /// Throughput (docs/sec)
    pub throughput: f64,

    /// Jaccard threshold used
    pub threshold: f64,
}

impl DedupResults {
    /// Calculate deduplication rate
    pub fn dedup_rate(&self) -> f64 {
        if self.total_docs == 0 {
            0.0
        } else {
            (self.total_duplicates as f64 / self.total_docs as f64) * 100.0
        }
    }

    /// Get unique documents count
    pub fn unique_docs(&self) -> usize {
        self.total_docs - self.total_duplicates
    }
}

/// Sample cluster for display
#[derive(Debug, Clone)]
pub struct ClusterSample {
    pub cluster_id: usize,
    pub size: usize,
    pub doc_ids: Vec<usize>,
    pub representative_text: String,
}

/// Result viewer component
pub struct ResultViewer {
    /// Results to display
    results: DedupResults,

    /// Scroll offset for table
    scroll_offset: usize,

    /// Selected view (0 = summary, 1 = clusters, 2 = distribution)
    selected_view: usize,
}

impl ResultViewer {
    /// Create new result viewer
    pub fn new(results: DedupResults) -> Self {
        Self {
            results,
            scroll_offset: 0,
            selected_view: 0,
        }
    }

    /// Scroll up
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Scroll down
    pub fn scroll_down(&mut self, max: usize) {
        self.scroll_offset = (self.scroll_offset + 1).min(max);
    }

    /// Switch view
    pub fn switch_view(&mut self, view: usize) {
        self.selected_view = view;
        self.scroll_offset = 0;
    }

    /// Handle keyboard input
    pub fn handle_key(&mut self, key: crossterm::event::KeyCode) -> ResultViewerAction {
        use crossterm::event::KeyCode;

        match key {
            KeyCode::Up => {
                self.scroll_up();
                ResultViewerAction::Continue
            }
            KeyCode::Down => {
                let max = self.results.sample_clusters.len().saturating_sub(1);
                self.scroll_down(max);
                ResultViewerAction::Continue
            }
            KeyCode::Char('1') => {
                self.switch_view(0);
                ResultViewerAction::Continue
            }
            KeyCode::Char('2') => {
                self.switch_view(1);
                ResultViewerAction::Continue
            }
            KeyCode::Char('3') => {
                self.switch_view(2);
                ResultViewerAction::Continue
            }
            KeyCode::Char('e') => ResultViewerAction::Export,
            KeyCode::Char('q') | KeyCode::Esc => ResultViewerAction::Exit,
            _ => ResultViewerAction::Continue,
        }
    }

    /// Render result viewer to frame
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Create layout: header + content + footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Header (summary stats)
                Constraint::Min(10),   // Content (table/chart)
                Constraint::Length(3), // Footer (controls)
            ])
            .split(area);

        // Render header (summary statistics)
        self.render_header(frame, chunks[0]);

        // Render content based on selected view
        match self.selected_view {
            0 => self.render_summary_table(frame, chunks[1]),
            1 => self.render_cluster_details(frame, chunks[1]),
            2 => self.render_distribution_chart(frame, chunks[1]),
            _ => {}
        }

        // Render footer (controls)
        self.render_footer(frame, chunks[2]);
    }

    /// Render header with summary statistics
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let stats_text = vec![
            Line::from(vec![
                Span::styled("Total Docs: ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{}", self.results.total_docs)),
                Span::raw("  |  "),
                Span::styled("Clusters: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{}", self.results.num_clusters)),
                Span::raw("  |  "),
                Span::styled("Duplicates: ", Style::default().fg(Color::Red)),
                Span::raw(format!("{}", self.results.total_duplicates)),
            ]),
            Line::from(vec![
                Span::styled("Dedup Rate: ", Style::default().fg(Color::Magenta)),
                Span::raw(format!("{:.2}%", self.results.dedup_rate())),
                Span::raw("  |  "),
                Span::styled("Throughput: ", Style::default().fg(Color::Green)),
                Span::raw(format!("{:.0} docs/sec", self.results.throughput)),
                Span::raw("  |  "),
                Span::styled("Elapsed: ", Style::default().fg(Color::Blue)),
                Span::raw(format!("{:.2}s", self.results.elapsed_seconds)),
            ]),
            Line::from(vec![
                Span::styled("Threshold: ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{:.2}", self.results.threshold)),
                Span::raw("  |  "),
                Span::styled("Unique: ", Style::default().fg(Color::Green)),
                Span::raw(format!("{}", self.results.unique_docs())),
            ]),
        ];

        let stats = Paragraph::new(stats_text).block(Block::default().borders(Borders::ALL).title("Summary"));
        frame.render_widget(stats, area);
    }

    /// Render summary table
    fn render_summary_table(&self, frame: &mut Frame, area: Rect) {
        let header_cells = ["Metric", "Value"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
        let header = Row::new(header_cells).height(1).bottom_margin(1);

        let rows = vec![
            Row::new(vec![
                Cell::from("Total Documents"),
                Cell::from(format!("{}", self.results.total_docs)),
            ]),
            Row::new(vec![
                Cell::from("Unique Documents"),
                Cell::from(format!("{}", self.results.unique_docs())),
            ]),
            Row::new(vec![
                Cell::from("Duplicate Clusters"),
                Cell::from(format!("{}", self.results.num_clusters)),
            ]),
            Row::new(vec![
                Cell::from("Total Duplicates"),
                Cell::from(format!("{}", self.results.total_duplicates)),
            ]),
            Row::new(vec![
                Cell::from("Deduplication Rate"),
                Cell::from(format!("{:.2}%", self.results.dedup_rate())),
            ]),
            Row::new(vec![
                Cell::from("Jaccard Threshold"),
                Cell::from(format!("{:.2}", self.results.threshold)),
            ]),
            Row::new(vec![
                Cell::from("Processing Time"),
                Cell::from(format!("{:.2} seconds", self.results.elapsed_seconds)),
            ]),
            Row::new(vec![
                Cell::from("Throughput"),
                Cell::from(format!("{:.0} docs/sec", self.results.throughput)),
            ]),
        ];

        let table = Table::new(rows, [Constraint::Percentage(50), Constraint::Percentage(50)])
            .header(header)
            .block(Block::default().borders(Borders::ALL).title("Summary Metrics"));

        frame.render_widget(table, area);
    }

    /// Render cluster details
    fn render_cluster_details(&self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .results
            .sample_clusters
            .iter()
            .skip(self.scroll_offset)
            .map(|cluster| {
                let content = vec![
                    Line::from(vec![
                        Span::styled(
                            format!("Cluster #{} ", cluster.cluster_id),
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!("(size: {})", cluster.size), Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from(vec![
                        Span::raw("  Doc IDs: "),
                        Span::styled(format!("{:?}", cluster.doc_ids), Style::default().fg(Color::Green)),
                    ]),
                    Line::from(vec![
                        Span::raw("  Sample: "),
                        Span::raw(cluster.representative_text.chars().take(60).collect::<String>()),
                        if cluster.representative_text.len() > 60 {
                            Span::raw("...")
                        } else {
                            Span::raw("")
                        },
                    ]),
                    Line::from(""),
                ];

                ListItem::new(content)
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Cluster Details (↑/↓ to scroll)"),
        );

        frame.render_widget(list, area);
    }

    /// Render cluster size distribution chart
    fn render_distribution_chart(&self, frame: &mut Frame, area: Rect) {
        // Aggregate cluster sizes
        let mut size_counts: Vec<(usize, usize)> = self
            .results
            .cluster_distribution
            .iter()
            .map(|(size, count)| (*size, *count))
            .collect();
        size_counts.sort_by_key(|(size, _)| *size);

        // Take top 10 sizes for display
        size_counts.truncate(10);

        if size_counts.is_empty() {
            let empty = Paragraph::new("No cluster distribution data")
                .block(Block::default().borders(Borders::ALL).title("Distribution"));
            frame.render_widget(empty, area);
            return;
        }

        // Create bar chart data
        let bars: Vec<Bar> = size_counts
            .iter()
            .map(|(size, count)| {
                Bar::default()
                    .label(Line::from(format!("size {}", size)))
                    .value(*count as u64)
                    .style(Style::default().fg(Color::Cyan))
            })
            .collect();

        let bar_group = BarGroup::default().bars(&bars);

        let chart = BarChart::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Cluster Size Distribution"),
            )
            .data(bar_group)
            .bar_width(5)
            .bar_gap(1)
            .value_style(Style::default().fg(Color::Yellow))
            .label_style(Style::default().fg(Color::White));

        frame.render_widget(chart, area);
    }

    /// Render footer with controls
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let footer_text = vec![Line::from(vec![
            Span::styled("1", Style::default().fg(Color::Yellow)),
            Span::raw(": Summary | "),
            Span::styled("2", Style::default().fg(Color::Yellow)),
            Span::raw(": Clusters | "),
            Span::styled("3", Style::default().fg(Color::Yellow)),
            Span::raw(": Distribution | "),
            Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
            Span::raw(": Scroll | "),
            Span::styled("e", Style::default().fg(Color::Yellow)),
            Span::raw(": Export | "),
            Span::styled("q", Style::default().fg(Color::Yellow)),
            Span::raw(": Quit"),
        ])];

        let footer = Paragraph::new(footer_text).block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, area);
    }
}

/// Result viewer action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultViewerAction {
    Continue,
    Export,
    Exit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_results() {
        let results = DedupResults {
            total_docs: 1000,
            num_clusters: 50,
            total_duplicates: 200,
            cluster_distribution: HashMap::new(),
            sample_clusters: Vec::new(),
            elapsed_seconds: 10.0,
            throughput: 100.0,
            threshold: 0.85,
        };

        assert_eq!(results.unique_docs(), 800);
        assert_eq!(results.dedup_rate(), 20.0);
    }

    #[test]
    fn test_result_viewer_scroll() {
        let results = DedupResults {
            total_docs: 100,
            num_clusters: 10,
            total_duplicates: 20,
            cluster_distribution: HashMap::new(),
            sample_clusters: vec![ClusterSample {
                cluster_id: 1,
                size: 5,
                doc_ids: vec![1, 2, 3, 4, 5],
                representative_text: "Sample text".to_string(),
            }],
            elapsed_seconds: 5.0,
            throughput: 20.0,
            threshold: 0.85,
        };

        let mut viewer = ResultViewer::new(results);
        assert_eq!(viewer.scroll_offset, 0);

        viewer.scroll_down(10);
        assert_eq!(viewer.scroll_offset, 1);

        viewer.scroll_up();
        assert_eq!(viewer.scroll_offset, 0);
    }

    #[test]
    fn test_view_switching() {
        let results = DedupResults {
            total_docs: 100,
            num_clusters: 10,
            total_duplicates: 20,
            cluster_distribution: HashMap::new(),
            sample_clusters: Vec::new(),
            elapsed_seconds: 5.0,
            throughput: 20.0,
            threshold: 0.85,
        };

        let mut viewer = ResultViewer::new(results);
        assert_eq!(viewer.selected_view, 0);

        viewer.switch_view(1);
        assert_eq!(viewer.selected_view, 1);

        viewer.switch_view(2);
        assert_eq!(viewer.selected_view, 2);
    }
}
