pub fn get_memory_usage() -> f64 {
    // use ps -o rss= -p <pid> to get memory usage. return in MB
    let pid = std::process::id();
    let mem_usage = std::process::Command::new("ps")
        .arg("-o rss=")
        .arg("-p")
        .arg(pid.to_string())
        .output()
        .expect("failed to execute process");
    let mem_usage = String::from_utf8(mem_usage.stdout).unwrap();
    let mem_usage = mem_usage.trim().parse::<f64>().unwrap() / 1000.0;
    return mem_usage;
}
