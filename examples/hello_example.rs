//! Example demonstrating how to use symgraph

fn main() {
    println!("Hello from symgraph example!");
    greet("World");
    calculate_sum();
}

fn greet(name: &str) {
    println!("Hello, {}!", name);
}

fn calculate_sum() {
    let numbers = vec![1, 2, 3, 4, 5];
    let sum: i32 = numbers.iter().sum();
    println!("Sum: {}", sum);
}
