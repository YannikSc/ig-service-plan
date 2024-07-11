use indicatif::style::TemplateError;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug, clap::Parser)]
#[command(name = "service-plan")]
pub struct Args {
    #[arg(help = "The path to the plan's YAML file")]
    pub plan: PathBuf,
    #[arg(help = "The project in which the plan is applied")]
    pub project: String,
    #[arg(help = "The subproject in which the plan is applied")]
    pub subproject: String,
    #[arg(help = "The environment on which the plan should be applied")]
    pub environment: String,
}

pub fn show_spinner(message: &str) -> anyhow::Result<impl FnOnce()> {
    Ok(animate_spinner(build_spinner(message)?))
}

pub fn build_spinner(message: &str) -> Result<ProgressBar, TemplateError> {
    let style = ProgressStyle::with_template("{spinner:.yellow} {wide_msg:.cyan} {prefix}")?
        .tick_chars("|/-\\/-\\/");
    let progress_bar = ProgressBar::new_spinner();
    progress_bar.set_message(message.to_string());
    progress_bar.set_style(style);

    Ok(progress_bar)
}

pub fn animate_spinner(progress_bar: ProgressBar) -> impl FnOnce() {
    let (tx, rx) = std::sync::mpsc::channel::<bool>();

    let handle = std::thread::spawn(move || {
        while !rx
            .recv_timeout(Duration::from_millis(150))
            .unwrap_or_default()
        {
            progress_bar.inc(1);
            let dots = ".".repeat(progress_bar.position() as usize % 4);
            progress_bar.set_prefix(dots);
        }

        progress_bar.set_style(
            ProgressStyle::with_template(" ✔️  {wide_msg:.dim}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        progress_bar.finish();
    });

    move || {
        tx.send(true).ok();
        handle.join().ok();
    }
}
