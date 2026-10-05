// The stock catalogue as the game builds it from the registry: every item, its
// worked-out reference price, how it stows and its market category.
//
//     cargo run --release -p universe-world --example stock_dump

fn main() {
    let c = universe_world::goods::catalog();
    println!("{} items, {} with a category", c.len(), c.iter().filter(|i| i.category.is_some()).count());
    for i in &c {
        println!("{:<34} {:<28} {:>10.1} cr/t {:>5.2} t/m3 {}", i.key, i.name, i.price, i.bulk_density, i.category.map_or("-", |c| c.name()));
    }
}
