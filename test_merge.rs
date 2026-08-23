use std::process::Command;

fn main() {
    let output = Command::new("ffmpeg")
        .arg("-h")
        .output()
        .unwrap();
    println!("status: {}", output.status);
}
