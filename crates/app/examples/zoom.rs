//! Debug tool: crops and magnifies a screenshot to inspect the rendering.
//! cargo run --release --example zoom -- in.png out.png X Y W H SCALE
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let img = image::open(&a[1]).unwrap().to_rgba8();
    let (x, y, w, h): (u32, u32, u32, u32) =
        (a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap(), a[6].parse().unwrap());
    let scale: u32 = a[7].parse().unwrap();
    let sub = image::imageops::crop_imm(&img, x, y, w, h).to_image();
    let big = image::imageops::resize(
        &sub,
        w * scale,
        h * scale,
        image::imageops::FilterType::Nearest,
    );
    big.save(&a[2]).unwrap();
    println!("{} -> {}x{}", a[2], w * scale, h * scale);
}
