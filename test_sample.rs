// This is a comment line - should be skipped
fn main() {
    // Another comment - skip this
    let x = 42;
    let y = x + 1; // inline comment
    println!("{}", y);

    /* Block comment
       spanning multiple lines
       should all be skipped */
    let result = x * y;
    println!("Result: {}", result);
}
