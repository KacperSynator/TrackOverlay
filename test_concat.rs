use std::process::Command;
use std::fs::File;
use std::io::Write;

fn main() {
    let mut file = File::create("concat_test.txt").unwrap();
    writeln!(file, "file 'does_not_exist.mp4'").unwrap();

    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg("concat_test.txt")
        .arg("-c")
        .arg("copy")
        .arg("-map")
        .arg("0")
        .arg("output.mp4")
        .output()
        .unwrap();
    println!("status: {}", output.status);
    println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
}
