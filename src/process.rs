use std::{collections::VecDeque, ffi::OsStr, path::Path, process::Stdio, sync::Arc};

use anyhow::{Context, Result};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::Mutex,
};

use crate::{error::DeployError, ui};

/// 流式执行命令所需的输入。
#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub stage: &'static str,
    pub program: String,
    pub args: Vec<String>,
    pub display_override: Option<String>,
    pub workdir: Option<std::path::PathBuf>,
    pub envs: Vec<(String, String)>,
    pub stdin_text: Option<String>,
}

impl CommandSpec {
    /// 以人类可读格式展示命令。
    pub fn display_command(&self) -> String {
        if let Some(display_override) = &self.display_override {
            return display_override.clone();
        }
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

/// 实时执行命令，并把 stdout/stderr 原样转发到当前终端。
///
/// 这样可以保留 git clone、maven package、docker build/push 的原生命令输出。
pub async fn run_streamed(spec: &CommandSpec) -> Result<()> {
    ui::print_info(spec.stage, &format!("执行命令: {}", spec.display_command()));
    let spinner = ui::start_spinner(spec.stage, &format!("运行中: {}", spec.program));

    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if spec.stdin_text.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });

    if let Some(workdir) = &spec.workdir {
        command.current_dir(workdir);
    }

    for (key, value) in &spec.envs {
        command.env(key, value);
    }

    let mut child = command
        .spawn()
        .with_context(|| format!("启动命令失败：{}", spec.display_command()))?;

    if let Some(stdin_text) = &spec.stdin_text {
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(stdin_text.as_bytes())
                .await
                .context("向子进程写入 stdin 失败")?;
            stdin.write_all(b"\n").await.context("写入换行失败")?;
        }
    }

    let stderr_tail: Arc<Mutex<VecDeque<String>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(8)));

    let stdout_task = child
        .stdout
        .take()
        .map(|stdout| tokio::spawn(pipe_reader(spec.stage, false, BufReader::new(stdout), None)));

    let stderr_task = child.stderr.take().map(|stderr| {
        tokio::spawn(pipe_reader(
            spec.stage,
            true,
            BufReader::new(stderr),
            Some(stderr_tail.clone()),
        ))
    });

    let status = child.wait().await.context("等待子进程退出失败")?;

    if let Some(task) = stdout_task {
        task.await.context("读取 stdout 任务失败")??;
    }
    if let Some(task) = stderr_task {
        task.await.context("读取 stderr 任务失败")??;
    }

    if status.success() {
        spinner.finish_with_message(format!("{} 已完成", spec.stage));
        return Ok(());
    }

    spinner.finish_with_message(format!("{} 失败", spec.stage));
    let stderr_tail = stderr_tail.lock().await;
    let stderr_tail = if stderr_tail.is_empty() {
        None
    } else {
        Some(stderr_tail.iter().cloned().collect::<Vec<_>>().join(" | "))
    };

    Err(DeployError::CommandFailed {
        stage: spec.stage.to_string(),
        command: spec.display_command(),
        exit_code: status.code(),
        stderr_tail,
    }
    .into())
}

async fn pipe_reader<R>(
    stage: &str,
    is_stderr: bool,
    reader: BufReader<R>,
    stderr_tail: Option<Arc<Mutex<VecDeque<String>>>>,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await.context("读取命令输出失败")? {
        if is_stderr {
            eprintln!("[{}] {} {}", ui::timestamp(), stage, line);
            if let Some(stderr_tail) = &stderr_tail {
                let mut guard = stderr_tail.lock().await;
                if guard.len() >= 8 {
                    guard.pop_front();
                }
                guard.push_back(line);
            }
        } else {
            println!("[{}] {} {}", ui::timestamp(), stage, line);
        }
    }
    Ok(())
}

/// 使用 `which` 探测外部命令是否可用。
pub fn require_command<S: AsRef<OsStr>>(program: S) -> Result<std::path::PathBuf> {
    let program = program.as_ref();
    which::which(program).map_err(|_| {
        DeployError::MissingTool {
            program: program.to_string_lossy().to_string(),
        }
        .into()
    })
}

/// 把路径转换成可供命令行调用的字符串。
pub fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
