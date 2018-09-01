pub fn index_html() -> &'static str {
    println!("IN INDEX");
    return include_str!("resources/html/index.html");
}
