//! Compte les pixels « clairs » dans une bande, pour décider si du texte y est
//! dessiné sans avoir à regarder l'image.
//! band -- img.png X0 X1 Y0 Y1
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let img = image::open(&a[1]).unwrap().to_rgba8();
    let p = |i: usize| -> u32 { a[i].parse().unwrap() };
    let (x0, x1, y0, y1) = (p(2), p(3), p(4), p(5));
    let mut lit = 0;
    for y in y0..y1.min(img.height()) {
        for x in x0..x1.min(img.width()) {
            let px = img.get_pixel(x, y);
            // Le fond des rangées tourne autour de 21-40 ; le texte de méta est
            // nettement plus clair.
            if px[0] as u32 + px[1] as u32 + px[2] as u32 > 240 {
                lit += 1;
            }
        }
    }
    println!("{lit} pixels clairs dans [{x0}..{x1}]x[{y0}..{y1}]");
}
