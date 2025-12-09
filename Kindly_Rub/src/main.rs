use kindly_rub::KindlyRubAppCapsule;

fn main() {
    let app = KindlyRubAppCapsule::new();
    println!(
        "Kindly Rub initialized. Timeline blocks: {}",
        app.timeline.len()
    );
}
