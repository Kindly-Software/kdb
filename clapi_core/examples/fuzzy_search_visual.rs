//! Visual demonstration of fuzzy search scoring
//!
//! Shows how the fuzzy matching algorithm scores different queries.

fn main() {
    // Simplified fuzzy scoring for demo
    fn fuzzy_score(query: &str, target: &str) -> u8 {
        if query.is_empty() {
            return 100;
        }
        let query_lower = query.to_ascii_lowercase();
        let target_lower = target.to_ascii_lowercase();
        if target_lower == query_lower {
            100
        } else if target_lower.starts_with(&query_lower) {
            90
        } else if target_lower.contains(&query_lower) {
            50
        } else {
            0
        }
    }

    println!("Fuzzy Search Visual Demo\n");
    println!("Commands: audit, budget, cache, clear, config, doctor, help, metrics, profile, providers, start, stop\n");

    let commands = vec![
        "audit", "budget", "cache", "clear", "config", "doctor",
        "help", "metrics", "profile", "providers", "start", "stop"
    ];

    let test_queries = vec![
        ("", "Empty query (show all)"),
        ("aud", "Prefix match"),
        ("audit", "Exact match"),
        ("dit", "Contains match"),
        ("met", "Metrics prefix"),
        ("pro", "Two matches (profile, providers)"),
        ("cache", "Exact match"),
        ("xyz", "No matches"),
    ];

    for (query, description) in test_queries {
        println!("Query: '{}' - {}", query, description);
        println!("{}", "-".repeat(50));

        let mut results: Vec<_> = commands.iter()
            .map(|&cmd| (cmd, fuzzy_score(query, cmd)))
            .filter(|(_, score)| *score > 0)
            .collect();

        results.sort_by(|a, b| b.1.cmp(&a.1));

        if results.is_empty() {
            println!("  No matches\n");
        } else {
            for (i, (cmd, score)) in results.iter().enumerate() {
                let bar = "█".repeat((score / 10) as usize);
                println!("  {}. {:<12} [{}] {}", i + 1, cmd, bar, score);
            }
            println!();
        }
    }

    println!("Scoring System:");
    println!("  100 points = Exact match");
    println!("   90 points = Prefix match (starts with query)");
    println!("   50 points = Contains match (query somewhere in command)");
    println!("    0 points = No match (filtered out)");
}
