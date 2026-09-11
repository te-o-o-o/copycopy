//! Sonde : valeurs RGBA de quelques pixels. Sert à savoir si une capture de
//! fenêtre contient vraiment de la transparence dans les coins.
//! pixel -- img.png X,Y X,Y ...
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let img = image::open(&a[1]).unwrap().to_rgba8();
    println!("{}x{}", img.width(), img.height());
    for spec in &a[2..] {
        let (x, y) = spec.split_once(',').unwrap();
        let (x, y): (u32, u32) = (x.parse().unwrap(), y.parse().unwrap());
        let p = img.get_pixel(x, y);
        println!("({x:>4},{y:>4}) rgba({:>3},{:>3},{:>3},{:>3})", p[0], p[1], p[2], p[3]);
    }
}
