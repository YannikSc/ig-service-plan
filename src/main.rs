use anyhow::Context;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use term_table::row::Row;
use term_table::table_cell::TableCell;

use crate::cli::show_spinner;
use crate::config::ServicePlan;
use crate::plan_processor::ServicePlanProcessor;

mod cli;
mod config;
mod plan_processor;
mod processable_value;

#[derive(Clone, Debug, serde::Deserialize)]
struct BriefServerObject {
    hostname: String,
    servertype: String,
    #[serde(default)]
    state: String,
}

async fn show_unmanaged_objects(
    managed_objects: Vec<String>,
    project: String,
    subproject: String,
    environment: String,
) -> anyhow::Result<()> {
    let all_selected_objects = adminapi::query::Query::builder()
        .filter("project", project.clone())
        .filter("subproject", subproject.clone())
        .filter("environment", environment.clone())
        .restrict(["hostname", "servertype", "state"])
        .build()
        .request_typed::<BriefServerObject>()
        .await?
        .all()
        .into_iter()
        .filter(|obj| !managed_objects.contains(&obj.attributes.hostname))
        .collect::<Vec<_>>();

    if all_selected_objects.is_empty() {
        return Ok(());
    }

    let header_style = console::Style::new().bold();
    let mut unmanaged_object_table = term_table::Table::new();
    unmanaged_object_table.add_row(Row::new(vec![
        TableCell::new(header_style.apply_to("hostname")),
        TableCell::new(header_style.apply_to("servertype")),
        TableCell::new(header_style.apply_to("state")),
    ]));
    for obj in all_selected_objects {
        unmanaged_object_table.add_row(Row::new(vec![
            TableCell::new(obj.attributes.hostname),
            TableCell::new(obj.attributes.servertype),
            TableCell::new(obj.attributes.state),
        ]));
    }

    println!("\nOther (unmanaged) objects with the given selector (project={project} subproject={subproject} environment={environment}):");

    println!("{}", unmanaged_object_table.render());

    println!();

    Ok(())
}

async fn apply(args: crate::cli::Apply) -> anyhow::Result<()> {
    let stop = show_spinner("Reading service plan")?;
    let crate::cli::Apply {
        project,
        subproject,
        environment,
        ..
    } = args.clone();
    let plan: ServicePlan = serde_yml::from_reader(std::fs::File::open(args.plan)?)?;
    let mut processor = ServicePlanProcessor::new(plan);
    stop();

    let stop = show_spinner("Planning the service landscape")?;
    processor
        .project(args.project)
        .subproject(args.subproject)
        .environment(args.environment);
    let objects = processor.get_unrelational_resources().await?;
    stop();

    let header_style = console::Style::new().bold();
    let mut table = term_table::Table::new();
    table.add_row(Row::new(vec![
        TableCell::new(header_style.apply_to("hostname")),
        TableCell::new(header_style.apply_to("servertype")),
        TableCell::new(header_style.apply_to("Action")),
    ]));

    println!("\n\nThis action will create the following objects:\n");
    for object in &objects {
        let serde_json::Value::String(servertype) = object.get("servertype") else {
            continue;
        };
        let serde_json::Value::String(hostname) = object.get("hostname") else {
            continue;
        };

        table.add_row(Row::new(vec![
            TableCell::new(hostname),
            TableCell::new(servertype),
            if object.is_new() {
                TableCell::new("Create")
            } else if object.has_changes() {
                TableCell::new("Update")
            } else {
                TableCell::new("No action")
            },
        ]));
    }

    table.add_row(Row::new(vec![
        TableCell::new(header_style.apply_to("Total")),
        TableCell::new(header_style.apply_to(objects.len())),
    ]));

    println!("{}", table.render());

    show_unmanaged_objects(
        objects
            .iter()
            .map(|obj| {
                obj.get("hostname")
                    .as_str()
                    .map(ToString::to_string)
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>(),
        project,
        subproject,
        environment,
    )
    .await?;

    if !objects.iter().any(|obj| obj.is_new() || obj.has_changes()) {
        println!("No pending changes");

        return Ok(());
    }

    let select = dialoguer::Select::new()
        .with_prompt("Continue")
        .item("No")
        .item("Yes")
        .default(0);

    if select.interact()? == 0 {
        println!("Aborting.");

        return Ok(());
    }

    println!();

    let objects = objects
        .into_iter()
        .filter(|obj| obj.is_new() || obj.has_changes())
        .collect::<Vec<_>>();

    let progress_style =
        ProgressStyle::with_template("{msg:.white.bold} [{wide_bar:.yellow}] {pos}/{len}")?
            .progress_chars("#>=");
    let progress_done_style =
        ProgressStyle::with_template("{msg:.dim} [{wide_bar:.cyan}] {pos}/{len}")?
            .progress_chars("#>=");
    let progress = ProgressBar::new(objects.len() as u64)
        .with_message("Creating objects")
        .with_style(progress_style.clone());

    let servers = futures::future::try_join_all(objects.into_iter().map(|object| {
        let progress = progress.clone();

        Box::pin(async move {
            let hostname = object.get("hostname");
            let hostname = hostname.as_str().unwrap_or_default();
            let result = object
                .commit()
                .await
                .context(format!("Creating object {hostname:?}"));
            progress.inc(1);

            result
        })
    }))
    .await?;
    progress.set_style(progress_done_style.clone());
    progress.finish();

    let progress = ProgressBar::new(servers.len() as u64)
        .with_message("Saving relations")
        .with_style(progress_style.clone());

    futures::future::try_join_all(servers.into_iter().map(|mut server| {
        let progress = progress.clone();

        Box::pin(async move {
            let result = server.commit().await;
            progress.inc(1);

            result
        })
    }))
    .await?;
    progress.set_style(progress_done_style);
    progress.finish();

    println!("\n\nDone. Enjoy your system!");

    Ok(())
}

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    // std::env::set_var(adminapi::config::ENV_NAME_BASE_URL, "http://127.0.0.1:8080");

    let args = cli::Args::parse();

    match args.subcommand {
        cli::Subcommands::Apply(args) => apply(args).await,
    }
}
