mod util;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use docsuite_convert::{ConvertOptions, ImageToPdfOptions, convert};
use docsuite_core::CancelToken;
use docsuite_ocr::predict_type;
use docsuite_pdf::{
    ImagePageFit, PageRanges, Pdf, Rgb, SignatureStamp, add_image, add_text, crop, delete_pages,
    merge, pdf_info, reorder, rotate, split, split_every, visible_signature,
};
use util::{
    ensure_output, ensure_output_dir, field_type_label, parse_order, parse_pages, parse_rect,
    part_path, progress_sink,
};

#[derive(Parser)]
#[command(name = "docapp", version, about = "Waghitech Docapp")]
struct Cli {
    #[arg(short, long, global = true)]
    quiet: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Info(InfoCmd),
    Split(SplitCmd),
    Merge(MergeCmd),
    Rotate(RotateCmd),
    Crop(CropCmd),
    Reorder(ReorderCmd),
    Delete(DeleteCmd),
    StampText(StampTextCmd),
    StampImage(StampImageCmd),
    SignVisible(SignVisibleCmd),
    DetectType(DetectTypeCmd),
    Convert(ConvertCmd),
}

#[derive(Args)]
struct ForceOpt {
    #[arg(long)]
    force: bool,
}

#[derive(Args)]
struct InfoCmd {
    file: PathBuf,
}

#[derive(Args)]
struct SplitCmd {
    input: PathBuf,
    #[arg(long)]
    ranges: Vec<String>,
    #[arg(long)]
    every: Option<u32>,
    #[arg(short, long)]
    output: PathBuf,
    #[command(flatten)]
    force: ForceOpt,
}

#[derive(Args)]
struct MergeCmd {
    inputs: Vec<PathBuf>,
    #[arg(short, long)]
    output: PathBuf,
    #[command(flatten)]
    force: ForceOpt,
}

#[derive(Args)]
struct RotateCmd {
    input: PathBuf,
    #[arg(long)]
    pages: String,
    #[arg(long)]
    degrees: i32,
    #[arg(short, long)]
    output: PathBuf,
    #[command(flatten)]
    force: ForceOpt,
}

#[derive(Args)]
struct CropCmd {
    input: PathBuf,
    #[arg(long)]
    pages: String,
    #[arg(long, value_name = "X,Y,W,H")]
    r#box: String,
    #[arg(short, long)]
    output: PathBuf,
    #[command(flatten)]
    force: ForceOpt,
}

#[derive(Args)]
struct ReorderCmd {
    input: PathBuf,
    #[arg(long)]
    order: String,
    #[arg(short, long)]
    output: PathBuf,
    #[command(flatten)]
    force: ForceOpt,
}

#[derive(Args)]
struct DeleteCmd {
    input: PathBuf,
    #[arg(long)]
    pages: String,
    #[arg(short, long)]
    output: PathBuf,
    #[command(flatten)]
    force: ForceOpt,
}

#[derive(Args)]
struct StampTextCmd {
    input: PathBuf,
    #[arg(long)]
    page: u32,
    #[arg(long, value_name = "X,Y")]
    at: String,
    #[arg(long, default_value_t = 12.0)]
    size: f64,
    #[arg(long)]
    text: String,
    #[arg(short, long)]
    output: PathBuf,
    #[command(flatten)]
    force: ForceOpt,
}

#[derive(Args)]
struct StampImageCmd {
    input: PathBuf,
    #[arg(long)]
    page: u32,
    #[arg(long, value_name = "X,Y,W,H")]
    rect: String,
    #[arg(long)]
    image: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[command(flatten)]
    force: ForceOpt,
}

#[derive(Args)]
struct SignVisibleCmd {
    input: PathBuf,
    #[arg(long)]
    page: u32,
    #[arg(long, value_name = "X,Y,W,H")]
    rect: String,
    #[arg(long)]
    name: String,
    #[arg(long)]
    reason: String,
    #[arg(long)]
    image: Option<PathBuf>,
    #[arg(short, long)]
    output: PathBuf,
    #[command(flatten)]
    force: ForceOpt,
}

#[derive(Args)]
struct DetectTypeCmd {
    text: String,
}

#[derive(Args)]
struct ConvertCmd {
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    fit_a4: bool,
    #[arg(long)]
    fit_letter: bool,
    #[command(flatten)]
    force: ForceOpt,
}

fn parse_xy(spec: &str) -> Result<(f64, f64)> {
    let mut parts = spec.split(',');
    let x: f64 = parts.next().context("x")?.trim().parse().context("x")?;
    let y: f64 = parts.next().context("y")?.trim().parse().context("y")?;
    Ok((x, y))
}

fn load_pdf(path: &Path) -> Result<Pdf> {
    Pdf::load(path).map_err(|e| anyhow::anyhow!("{e}"))
}

fn copy_pdf_workflow<F>(
    input: &Path,
    output: &Path,
    force: bool,
    quiet: bool,
    work: F,
) -> Result<()>
where
    F: FnOnce(&mut Pdf, &CancelToken, &Option<docsuite_core::ProgressSink>) -> Result<()>,
{
    ensure_output(output, &[input], force)?;
    let cancel = CancelToken::new();
    let progress = progress_sink(quiet);
    let mut pdf = load_pdf(input)?;
    work(&mut pdf, &cancel, &progress)?;
    pdf.save(output).context("save pdf")?;
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Info(cmd) => cmd_info(cmd),
        Commands::Split(cmd) => cmd_split(cmd, cli.quiet),
        Commands::Merge(cmd) => cmd_merge(cmd, cli.quiet),
        Commands::Rotate(cmd) => cmd_rotate(cmd, cli.quiet),
        Commands::Crop(cmd) => cmd_crop(cmd, cli.quiet),
        Commands::Reorder(cmd) => cmd_reorder(cmd, cli.quiet),
        Commands::Delete(cmd) => cmd_delete(cmd, cli.quiet),
        Commands::StampText(cmd) => cmd_stamp_text(cmd, cli.quiet),
        Commands::StampImage(cmd) => cmd_stamp_image(cmd, cli.quiet),
        Commands::SignVisible(cmd) => cmd_sign_visible(cmd, cli.quiet),
        Commands::DetectType(cmd) => cmd_detect_type(cmd),
        Commands::Convert(cmd) => cmd_convert(cmd, cli.quiet),
    }
}

fn cmd_info(cmd: InfoCmd) -> Result<()> {
    let pdf = load_pdf(&cmd.file)?;
    let info = pdf_info(&pdf).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("version: {}", info.version);
    println!("pages: {}", info.pages);
    println!("encrypted: {}", info.encrypted);
    for (i, size) in info.page_sizes.iter().enumerate() {
        println!("page {}: {:.1}x{:.1} pt", i + 1, size.width, size.height);
    }
    Ok(())
}

fn cmd_split(cmd: SplitCmd, quiet: bool) -> Result<()> {
    if cmd.ranges.is_empty() == cmd.every.is_none() {
        bail!("use --ranges and/or --every");
    }
    if !cmd.ranges.is_empty() && cmd.every.is_some() {
        bail!("use only one of --ranges or --every");
    }
    ensure_output_dir(&cmd.output, &cmd.input, cmd.force.force)?;
    let pdf = load_pdf(&cmd.input)?;
    let cancel = CancelToken::new();
    let progress = progress_sink(quiet);
    let stem = cmd
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("doc");
    let parts = if let Some(n) = cmd.every {
        split_every(&pdf, n, &cancel, &progress).map_err(|e| anyhow::anyhow!("{e}"))?
    } else {
        let ranges: Vec<PageRanges> = cmd
            .ranges
            .iter()
            .map(|r| PageRanges::parse(r).map_err(|e| anyhow::anyhow!("{e}")))
            .collect::<Result<_>>()?;
        split(&pdf, &ranges, &cancel, &progress).map_err(|e| anyhow::anyhow!("{e}"))?
    };
    for (i, mut part) in parts.into_iter().enumerate() {
        let path = part_path(&cmd.output, stem, i + 1);
        part.save(&path).context("save split part")?;
        if !quiet {
            println!("{}", path.display());
        }
    }
    Ok(())
}

fn cmd_merge(cmd: MergeCmd, quiet: bool) -> Result<()> {
    if cmd.inputs.len() < 2 {
        bail!("merge needs at least two inputs");
    }
    let input_refs: Vec<&Path> = cmd.inputs.iter().map(|p| p.as_path()).collect();
    ensure_output(&cmd.output, &input_refs, cmd.force.force)?;
    let pdfs = cmd
        .inputs
        .iter()
        .map(|p| load_pdf(p))
        .collect::<Result<Vec<_>>>()?;
    let cancel = CancelToken::new();
    let progress = progress_sink(quiet);
    let mut merged = merge(pdfs, &cancel, &progress).map_err(|e| anyhow::anyhow!("{e}"))?;
    merged.save(&cmd.output).context("save merge")?;
    Ok(())
}

fn cmd_rotate(cmd: RotateCmd, quiet: bool) -> Result<()> {
    copy_pdf_workflow(
        &cmd.input,
        &cmd.output,
        cmd.force.force,
        quiet,
        |pdf, cancel, progress| {
            let pages = parse_pages(&cmd.pages, pdf.page_count())?;
            rotate(pdf, &pages, cmd.degrees, cancel, progress).map_err(|e| anyhow::anyhow!("{e}"))
        },
    )
}

fn cmd_crop(cmd: CropCmd, quiet: bool) -> Result<()> {
    let rect = parse_rect(&cmd.r#box)?;
    copy_pdf_workflow(
        &cmd.input,
        &cmd.output,
        cmd.force.force,
        quiet,
        |pdf, cancel, progress| {
            let pages = parse_pages(&cmd.pages, pdf.page_count())?;
            crop(pdf, &pages, rect, false, cancel, progress).map_err(|e| anyhow::anyhow!("{e}"))
        },
    )
}

fn cmd_reorder(cmd: ReorderCmd, quiet: bool) -> Result<()> {
    copy_pdf_workflow(
        &cmd.input,
        &cmd.output,
        cmd.force.force,
        quiet,
        |pdf, cancel, progress| {
            let order = parse_order(&cmd.order)?;
            reorder(pdf, &order, cancel, progress).map_err(|e| anyhow::anyhow!("{e}"))
        },
    )
}

fn cmd_delete(cmd: DeleteCmd, quiet: bool) -> Result<()> {
    copy_pdf_workflow(
        &cmd.input,
        &cmd.output,
        cmd.force.force,
        quiet,
        |pdf, cancel, progress| {
            let pages = parse_pages(&cmd.pages, pdf.page_count())?;
            delete_pages(pdf, &pages, cancel, progress).map_err(|e| anyhow::anyhow!("{e}"))
        },
    )
}

fn cmd_stamp_text(cmd: StampTextCmd, quiet: bool) -> Result<()> {
    let (x, y) = parse_xy(&cmd.at)?;
    copy_pdf_workflow(
        &cmd.input,
        &cmd.output,
        cmd.force.force,
        quiet,
        |pdf, cancel, progress| {
            add_text(
                pdf,
                cmd.page,
                x,
                y,
                cmd.size,
                &cmd.text,
                Rgb(0.0, 0.0, 0.0),
                cancel,
                progress,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))
        },
    )
}

fn cmd_stamp_image(cmd: StampImageCmd, quiet: bool) -> Result<()> {
    let rect = parse_rect(&cmd.rect)?;
    let image_bytes = std::fs::read(&cmd.image).context("read stamp image")?;
    copy_pdf_workflow(
        &cmd.input,
        &cmd.output,
        cmd.force.force,
        quiet,
        |pdf, cancel, progress| {
            add_image(pdf, cmd.page, rect, &image_bytes, cancel, progress)
                .map_err(|e| anyhow::anyhow!("{e}"))
        },
    )
}

fn cmd_sign_visible(cmd: SignVisibleCmd, quiet: bool) -> Result<()> {
    let rect = parse_rect(&cmd.rect)?;
    let image = match &cmd.image {
        Some(path) => Some(std::fs::read(path).context("read signature image")?),
        None => None,
    };
    let name = cmd.name.clone();
    let reason = cmd.reason.clone();
    let page = cmd.page;
    copy_pdf_workflow(
        &cmd.input,
        &cmd.output,
        cmd.force.force,
        quiet,
        move |pdf, cancel, progress| {
            let stamp = SignatureStamp {
                name,
                reason,
                date: String::new(),
                image,
            };
            visible_signature(pdf, page, rect, stamp, cancel, progress)
                .map_err(|e| anyhow::anyhow!("{e}"))
        },
    )
}

fn cmd_detect_type(cmd: DetectTypeCmd) -> Result<()> {
    let guess = predict_type(&cmd.text);
    print!("{}", field_type_label(guess.field_type));
    if let Some(norm) = guess.normalized {
        println!(" {norm}");
    } else {
        println!();
    }
    Ok(())
}

fn cmd_convert(cmd: ConvertCmd, quiet: bool) -> Result<()> {
    ensure_output(&cmd.output, &[cmd.input.as_path()], cmd.force.force)?;
    let fit = if cmd.fit_letter {
        ImagePageFit::Letter
    } else if cmd.fit_a4 {
        ImagePageFit::A4
    } else {
        ImagePageFit::Natural72
    };
    let opts = ConvertOptions {
        image: ImageToPdfOptions { fit },
        ..ConvertOptions::default()
    };
    let cancel = CancelToken::new();
    let progress = progress_sink(quiet);
    convert(&cmd.input, &cmd.output, &opts, &cancel, &progress).map_err(|e| anyhow::anyhow!("{e}"))
}
