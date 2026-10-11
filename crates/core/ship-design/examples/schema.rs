fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&universe_ship_design::schema()).unwrap()
    );
}
