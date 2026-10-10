// The stock catalogue as the game builds it from the registry: every item, its
// worked-out reference price, its unit and how it stows, its market category;
// then how many recipes each module has.
//
//     cargo run --release -p universe-world --example stock_dump
fn main() {
    let c = universe_world::goods::catalog();
    println!("{} items, {} with a category", c.len(), c.iter().filter(|i| i.category.is_some()).count());
    for i in &c {
        println!("{:<34} {:<30} {:>12.1} cr/unit {:>10.1} kg/unit {:>5.2} t/m3 {}", i.key, i.name, i.price, i.mass, i.bulk_density, i.kind_name());
    }
    let content = universe_world::content::content();
    let mut modules: Vec<_> = content.recipes.iter().filter(|(_, r)| !r.is_empty()).map(|(k, r)| (k.clone(), r.len())).collect();
    modules.sort();
    for (k, n) in modules {
        println!("recipes {k:<32} {n}");
    }
}
