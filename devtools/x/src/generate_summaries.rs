// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::context::XContext;
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use clap::{ArgEnum, Parser};
use guppy::graph::{
    cargo::CargoOptions,
    feature::{FeatureSet, StandardFeatures},
};
use std::fs;

#[derive(Debug, Copy, Clone, ArgEnum)]
pub enum OutputFormat {
    Toml,
    Json,
}

impl std::str::FromStr for OutputFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "toml" => Ok(OutputFormat::Toml),
            "json" => Ok(OutputFormat::Json),
            _ => Err(anyhow::anyhow!("invalid output format: {}", s)),
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Toml => write!(f, "toml"),
            OutputFormat::Json => write!(f, "json"),
        }
    }
}

#[derive(Debug, Parser)]
pub struct Args {
    #[clap(name = "OUT_DIR")]
    /// Directory to output summaries to (default: target/summaries)
    out_dir: Option<Utf8PathBuf>,
    #[clap(name = "OUTPUT_FORMAT", default_value = "toml")]
    /// Output in text, toml or json
    output_format: OutputFormat,
}

impl Args {
    const DEFAULT_OUT_DIR: &'static str = "target/summaries";
}

pub fn run(args: Args, xctx: XContext) -> crate::Result<()> {
    let config = xctx.config();
    let summaries_config = config.summaries_config();
    let pkg_graph = xctx.core().package_graph()?;
    let subsets = xctx.core().subsets()?;

    let default_opts = summaries_config.default.to_cargo_options(pkg_graph)?;
    let full_opts = summaries_config.full.to_cargo_options(pkg_graph)?;

    let out_dir = args
        .out_dir
        .unwrap_or_else(|| xctx.core().project_root().join(Args::DEFAULT_OUT_DIR));

    fs::create_dir_all(&out_dir)?;

    // TODO: figure out a way to unify this with WorkspaceSubset.

    // Create summaries for:

    let mut summary_count = 0;

    // * default members (default features)
    // (note that we aren't using the build set from default_members() as it may have different
    // options)
    let initials = subsets.default_members().initials().clone();
    write_summary(
        "default",
        initials,
        &default_opts,
        &out_dir,
        args.output_format,
    )?;

    summary_count += 1;

    // * subsets (default features)
    for (name, subset) in subsets.iter() {
        let initials = subset.initials().clone();
        write_summary(name, initials, &default_opts, &out_dir, args.output_format)?;

        summary_count += 1;
    }

    // * full workspace set (all features)
    let initials = pkg_graph
        .resolve_workspace()
        .to_feature_set(StandardFeatures::All);
    write_summary("full", initials, &full_opts, &out_dir, args.output_format)?;

    summary_count += 1;

    println!("wrote {} summaries to {}", summary_count, out_dir);

    Ok(())
}

fn write_summary(
    name: &str,
    initials: FeatureSet<'_>,
    cargo_opts: &CargoOptions<'_>,
    out_dir: &Utf8Path,
    output_format: OutputFormat,
) -> crate::Result<()> {
    let build_set = initials.into_cargo_set(cargo_opts)?;
    let summary = build_set.to_summary(cargo_opts)?;

    let (out, path) = match output_format {
        OutputFormat::Json => {
            let out = serde_json::to_string(&summary)?;
            let summary_path = out_dir.join(format!("summary-{}.json", name));
            (out, summary_path)
        }
        OutputFormat::Toml => {
            let mut out = format!(
                "# Summary for Diem subset '{}'. @generated by x.\n\
                 # To regenerate, run 'cargo x generate-summaries'.\n\n",
                name
            );
            summary
                .write_to_string(&mut out)
                .with_context(|| format!("error while generating summary for '{}'", name))?;

            let summary_path = out_dir.join(format!("summary-{}.toml", name));

            (out, summary_path)
        }
    };

    fs::write(&path, &out).with_context(|| format!("error while writing summary file {}", path))
}
