use std::process::Command;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use shellexpand::tilde;

pub fn split_csv_file(
    file_path: &str, 
    output_prefix: &str, 
    header: bool, 
    split_count: usize
) -> Result<(), Box<dyn std::error::Error>> {
    // 展开可能存在的用户目录简写
    let file_path = tilde(file_path).to_string();
    let file_path = Path::new(&file_path);
    
    println!("file_path {:?}",file_path);

    // 计算文件的总行数
    let total_lines_output = Command::new("wc")
        .arg("-l")
        .arg(file_path)
        .output()?;
    let total_lines = String::from_utf8_lossy(&total_lines_output.stdout);
    let total_lines: usize = total_lines.trim().split_whitespace().next().unwrap().parse()?;
    
    
    println!("total_lines {:?}",total_lines);

    // 计算每个文件应该包含的行数
    let lines_per_file = (total_lines + split_count - 1) / split_count;
    
    println!("lines_per_file {:?}",lines_per_file);
    
    let tmp_dir = {
        let parent_dir = Path::new(file_path).parent().unwrap();
        let mut tmp_dir_path = parent_dir.to_path_buf();
        tmp_dir_path.push(format!("{}_tmp", parent_dir.file_name().unwrap().to_str().unwrap()));
        tmp_dir_path
    };
    
    if !tmp_dir.exists() {
        if let Err(err) = std::fs::create_dir_all(&tmp_dir) {
            println!("Error creating directory: {:?}", err);
        }
    }
    
    // 分割文件
    Command::new("split")
        .arg("-l")
        .arg(lines_per_file.to_string())
        .arg("--numeric-suffixes=1")
        .arg("--additional-suffix=.csv")
        .arg(file_path)
        .arg(format!("{}/{}", tmp_dir.to_str().unwrap(), output_prefix))

        // .arg(format!("{}/{}", tmp_dir.path().to_str().unwrap(), output_prefix))
        .status()?;
    
    println!("tmp_dir {:?}",tmp_dir.to_str().unwrap());

    if header {
        let header_content = Command::new("head")
            .arg("-n")
            .arg("1")
            .arg(file_path)
            .output()?;
        let header_content = String::from_utf8_lossy(&header_content.stdout);

        // for entry in tmp_dir.path().read_dir()? {
        for entry in tmp_dir.read_dir()? {
            let entry = entry?;
            let file_path = entry.path();
            let mut cmd = Command::new("sed");
            cmd.arg("-i").arg(format!("1i {}", header_content)).arg(file_path);
            let _ = cmd.status();
        }
    }

    // 将处理后的文件移动到原始文件所在目录
    for entry in tmp_dir.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        let new_path = file_path.parent().unwrap_or_else(|| Path::new("")).join(path.file_name().unwrap());
        std::fs::rename(path, new_path)?;
    }
    
    std::fs::remove_dir_all(tmp_dir)?;

    Ok(())
}

